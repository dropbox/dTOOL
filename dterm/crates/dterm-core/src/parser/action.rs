//! Parser actions and sink trait.

/// Action produced by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action<'a> {
    /// Print a character to the screen.
    Print(char),

    /// Execute a C0 or C1 control function.
    Execute(u8),

    /// Dispatch a CSI sequence.
    CsiDispatch {
        /// Numeric parameters (separated by ; or :)
        params: &'a [u16],
        /// Intermediate bytes (0x20-0x2F)
        intermediates: &'a [u8],
        /// Final byte (0x40-0x7E)
        final_byte: u8,
    },

    /// Dispatch an escape sequence.
    EscDispatch {
        /// Intermediate bytes
        intermediates: &'a [u8],
        /// Final byte
        final_byte: u8,
    },

    /// Dispatch an OSC sequence.
    OscDispatch {
        /// OSC parameters (separated by ;)
        params: &'a [&'a [u8]],
    },

    /// Hook a DCS sequence.
    DcsHook {
        /// Numeric parameters
        params: &'a [u16],
        /// Intermediate bytes
        intermediates: &'a [u8],
        /// Final byte
        final_byte: u8,
    },

    /// Put a byte into the DCS handler.
    DcsPut(u8),

    /// Unhook (terminate) a DCS sequence.
    DcsUnhook,

    /// Start an APC sequence (ESC _ or 0x9F).
    ApcStart,

    /// Put a byte into the APC handler.
    ApcPut(u8),

    /// End of an APC sequence.
    ApcEnd,
}

/// Trait for receiving parser actions.
///
/// Implement this trait to handle escape sequences from the parser.
pub trait ActionSink {
    /// Print a character to the screen at the cursor position.
    fn print(&mut self, c: char);

    /// Bulk print ASCII bytes (0x20-0x7E) in one call.
    ///
    /// Default implementation falls back to per-character `print()`.
    /// Implementations can override for better performance by avoiding
    /// per-character overhead for runs of ASCII text.
    ///
    /// # Safety
    /// The `data` slice is guaranteed to contain only printable ASCII (0x20-0x7E).
    #[inline]
    fn print_ascii_bulk(&mut self, data: &[u8]) {
        for &b in data {
            self.print(b as char);
        }
    }

    /// Execute a C0 control function (0x00-0x1F) or C1 (0x80-0x9F).
    fn execute(&mut self, byte: u8);

    /// A CSI sequence has been parsed.
    ///
    /// # Parameters
    /// - `params`: Numeric parameters (e.g., `[31]` for `ESC[31m`)
    /// - `intermediates`: Intermediate bytes (e.g., [?] for `ESC[?1h`)
    /// - `final_byte`: The final byte (e.g., 'm' for SGR)
    fn csi_dispatch(&mut self, params: &[u16], intermediates: &[u8], final_byte: u8);

    /// A CSI sequence has been parsed (extended version with subparameter info).
    ///
    /// This method is called instead of `csi_dispatch` when subparameters are present.
    /// The `subparam_mask` indicates which params were preceded by a colon (`:`)
    /// rather than a semicolon (`;`), marking them as subparameters.
    ///
    /// # Parameters
    /// - `params`: Numeric parameters
    /// - `intermediates`: Intermediate bytes
    /// - `final_byte`: The final byte
    /// - `subparam_mask`: Bitmask where bit `i` is set if `params[i]` is a subparameter
    ///
    /// Default implementation calls `csi_dispatch`, ignoring subparam info.
    fn csi_dispatch_with_subparams(
        &mut self,
        params: &[u16],
        intermediates: &[u8],
        final_byte: u8,
        _subparam_mask: u16,
    ) {
        self.csi_dispatch(params, intermediates, final_byte);
    }

    /// An escape sequence has been parsed.
    fn esc_dispatch(&mut self, intermediates: &[u8], final_byte: u8);

    /// An OSC sequence has been parsed.
    fn osc_dispatch(&mut self, params: &[&[u8]]);

    /// Start of a DCS sequence.
    fn dcs_hook(&mut self, params: &[u16], intermediates: &[u8], final_byte: u8);

    /// Data byte within a DCS sequence.
    fn dcs_put(&mut self, byte: u8);

    /// End of a DCS sequence.
    fn dcs_unhook(&mut self);

    /// Start of an APC (Application Program Command) sequence.
    ///
    /// Called when ESC _ or 0x9F is received. The sequence continues
    /// until ST (ESC \ or 0x9C) is received.
    fn apc_start(&mut self);

    /// Data byte within an APC sequence.
    fn apc_put(&mut self, byte: u8);

    /// End of an APC sequence.
    fn apc_end(&mut self);
}

/// Extended trait for batch-optimized action handling.
///
/// This trait adds `print_str` for handling runs of printable ASCII
/// in a single call, which can be more efficient than per-character
/// `print` calls.
pub trait BatchActionSink: ActionSink {
    /// Print a string of characters.
    ///
    /// The string is guaranteed to contain only printable ASCII (0x20-0x7E).
    /// This allows efficient batch processing without per-character overhead.
    fn print_str(&mut self, s: &str);
}

/// A sink that discards all actions.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl ActionSink for NullSink {
    fn print(&mut self, _: char) {}
    fn print_ascii_bulk(&mut self, _: &[u8]) {}
    fn execute(&mut self, _: u8) {}
    fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) {}
    fn esc_dispatch(&mut self, _: &[u8], _: u8) {}
    fn osc_dispatch(&mut self, _: &[&[u8]]) {}
    fn dcs_hook(&mut self, _: &[u16], _: &[u8], _: u8) {}
    fn dcs_put(&mut self, _: u8) {}
    fn dcs_unhook(&mut self) {}
    fn apc_start(&mut self) {}
    fn apc_put(&mut self, _: u8) {}
    fn apc_end(&mut self) {}
}

impl BatchActionSink for NullSink {
    fn print_str(&mut self, _: &str) {}
}
