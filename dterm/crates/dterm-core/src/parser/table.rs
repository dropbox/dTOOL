//! Compile-time generated transition table.
//!
//! Based on the vt100.net DEC ANSI parser state machine.
//! Reference: <https://vt100.net/emu/dec_ansi_parser>

use super::state::State;

/// Action to perform during a state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ActionType {
    /// No action
    #[default]
    None = 0,
    /// Print the character
    Print,
    /// Execute C0/C1 control
    Execute,
    /// Clear parameters and intermediates
    Clear,
    /// Collect intermediate byte
    Collect,
    /// Add digit to current parameter
    Param,
    /// Dispatch ESC sequence
    EscDispatch,
    /// Dispatch CSI sequence
    CsiDispatch,
    /// Hook DCS
    DcsHook,
    /// Put DCS byte
    DcsPut,
    /// Unhook DCS
    DcsUnhook,
    /// Start OSC
    OscStart,
    /// Put OSC byte
    OscPut,
    /// End OSC
    OscEnd,
    /// Ignore this byte
    Ignore,
    /// Start APC
    ApcStart,
    /// Put APC byte
    ApcPut,
    /// End APC
    ApcEnd,
}

/// A state transition entry.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Transition {
    /// Next state
    pub next_state: State,
    /// Action to perform
    pub action: ActionType,
}

impl Transition {
    /// Create a new transition.
    pub const fn new(next_state: State, action: ActionType) -> Self {
        Self { next_state, action }
    }
}

/// Helper to set transitions for a range of bytes in a state.
const fn set_range(
    table: &mut [[Transition; 256]; State::COUNT],
    state: State,
    start: u8,
    end: u8,
    transition: Transition,
) {
    let mut byte = start;
    while byte <= end {
        table[state as usize][byte as usize] = transition;
        if byte == 255 {
            break;
        }
        byte += 1;
    }
}

/// Generate the transition table at compile time.
///
/// This creates a 256 × 14 table (~7 KB) that maps
/// (current_state, input_byte) → (next_state, action).
///
/// Based on the vt100.net DEC ANSI parser state machine.
pub const fn generate_table() -> [[Transition; 256]; State::COUNT] {
    let mut table = [[Transition::new(State::Ground, ActionType::None); 256]; State::COUNT];

    // ========================
    // ANYWHERE transitions (handled first, can be overridden)
    // These apply to all states
    // ========================

    // C1 controls that transition anywhere
    // 0x18 (CAN), 0x1A (SUB) → execute + Ground
    // 0x1B (ESC) → Escape (clear)
    // 0x80-0x8F, 0x91-0x97, 0x99, 0x9A → execute + Ground
    // 0x9C (ST) → Ground
    // 0x90 (DCS) → DcsEntry (clear)
    // 0x9B (CSI) → CsiEntry (clear)
    // 0x9D (OSC) → OscString
    // 0x98, 0x9E, 0x9F (SOS, PM, APC) → SosPmApcString

    let mut state_idx = 0;
    while state_idx < State::COUNT {
        // CAN and SUB execute and go to ground
        table[state_idx][0x18] = Transition::new(State::Ground, ActionType::Execute);
        table[state_idx][0x1A] = Transition::new(State::Ground, ActionType::Execute);

        // ESC goes to Escape state with clear
        table[state_idx][0x1B] = Transition::new(State::Escape, ActionType::Clear);

        // C1 controls (8-bit)
        let mut c1 = 0x80u8;
        while c1 <= 0x8F {
            table[state_idx][c1 as usize] = Transition::new(State::Ground, ActionType::Execute);
            c1 += 1;
        }
        table[state_idx][0x90] = Transition::new(State::DcsEntry, ActionType::Clear); // DCS
        let mut c1 = 0x91u8;
        while c1 <= 0x97 {
            table[state_idx][c1 as usize] = Transition::new(State::Ground, ActionType::Execute);
            c1 += 1;
        }
        table[state_idx][0x98] = Transition::new(State::SosPmApcString, ActionType::None); // SOS
        table[state_idx][0x99] = Transition::new(State::Ground, ActionType::Execute);
        table[state_idx][0x9A] = Transition::new(State::Ground, ActionType::Execute);
        table[state_idx][0x9B] = Transition::new(State::CsiEntry, ActionType::Clear); // CSI
        table[state_idx][0x9C] = Transition::new(State::Ground, ActionType::None); // ST (terminates OSC/DCS/APC)
        table[state_idx][0x9D] = Transition::new(State::OscString, ActionType::OscStart); // OSC
        table[state_idx][0x9E] = Transition::new(State::SosPmApcString, ActionType::None); // PM
        table[state_idx][0x9F] = Transition::new(State::SosPmApcString, ActionType::ApcStart); // APC (C1)

        state_idx += 1;
    }

    // ========================
    // Ground state
    // ========================
    // 0x00-0x17: execute (C0 controls)
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x7F: print
    set_range(
        &mut table,
        State::Ground,
        0x00,
        0x17,
        Transition::new(State::Ground, ActionType::Execute),
    );
    table[State::Ground as usize][0x19] = Transition::new(State::Ground, ActionType::Execute);
    set_range(
        &mut table,
        State::Ground,
        0x1C,
        0x1F,
        Transition::new(State::Ground, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::Ground,
        0x20,
        0x7F,
        Transition::new(State::Ground, ActionType::Print),
    );

    // ========================
    // Escape state
    // ========================
    // 0x00-0x17: execute
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x2F: collect → EscapeIntermediate
    // 0x30-0x4F: esc_dispatch → Ground
    // 0x50 (P): → DcsEntry (clear)
    // 0x51-0x57: esc_dispatch → Ground
    // 0x58 (X): → SosPmApcString
    // 0x59: esc_dispatch → Ground
    // 0x5A: esc_dispatch → Ground
    // 0x5B ([): → CsiEntry (clear)
    // 0x5C (\): esc_dispatch → Ground
    // 0x5D (]): → OscString
    // 0x5E (^): → SosPmApcString (PM)
    // 0x5F (_): → SosPmApcString (APC)
    // 0x60-0x7E: esc_dispatch → Ground
    // 0x7F: ignore
    set_range(
        &mut table,
        State::Escape,
        0x00,
        0x17,
        Transition::new(State::Escape, ActionType::Execute),
    );
    table[State::Escape as usize][0x19] = Transition::new(State::Escape, ActionType::Execute);
    set_range(
        &mut table,
        State::Escape,
        0x1C,
        0x1F,
        Transition::new(State::Escape, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::Escape,
        0x20,
        0x2F,
        Transition::new(State::EscapeIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::Escape,
        0x30,
        0x4F,
        Transition::new(State::Ground, ActionType::EscDispatch),
    );
    table[State::Escape as usize][0x50] = Transition::new(State::DcsEntry, ActionType::Clear); // ESC P
    set_range(
        &mut table,
        State::Escape,
        0x51,
        0x57,
        Transition::new(State::Ground, ActionType::EscDispatch),
    );
    table[State::Escape as usize][0x58] = Transition::new(State::SosPmApcString, ActionType::None); // ESC X (SOS)
    table[State::Escape as usize][0x59] = Transition::new(State::Ground, ActionType::EscDispatch);
    table[State::Escape as usize][0x5A] = Transition::new(State::Ground, ActionType::EscDispatch);
    table[State::Escape as usize][0x5B] = Transition::new(State::CsiEntry, ActionType::Clear); // ESC [
    table[State::Escape as usize][0x5C] = Transition::new(State::Ground, ActionType::EscDispatch); // ESC \
    table[State::Escape as usize][0x5D] = Transition::new(State::OscString, ActionType::OscStart); // ESC ]
    table[State::Escape as usize][0x5E] = Transition::new(State::SosPmApcString, ActionType::None); // ESC ^ (PM)
    table[State::Escape as usize][0x5F] =
        Transition::new(State::SosPmApcString, ActionType::ApcStart); // ESC _ (APC)
    set_range(
        &mut table,
        State::Escape,
        0x60,
        0x7E,
        Transition::new(State::Ground, ActionType::EscDispatch),
    );
    table[State::Escape as usize][0x7F] = Transition::new(State::Escape, ActionType::Ignore);

    // ========================
    // EscapeIntermediate state
    // ========================
    // 0x00-0x17: execute
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x2F: collect
    // 0x30-0x7E: esc_dispatch → Ground
    // 0x7F: ignore
    set_range(
        &mut table,
        State::EscapeIntermediate,
        0x00,
        0x17,
        Transition::new(State::EscapeIntermediate, ActionType::Execute),
    );
    table[State::EscapeIntermediate as usize][0x19] =
        Transition::new(State::EscapeIntermediate, ActionType::Execute);
    set_range(
        &mut table,
        State::EscapeIntermediate,
        0x1C,
        0x1F,
        Transition::new(State::EscapeIntermediate, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::EscapeIntermediate,
        0x20,
        0x2F,
        Transition::new(State::EscapeIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::EscapeIntermediate,
        0x30,
        0x7E,
        Transition::new(State::Ground, ActionType::EscDispatch),
    );
    table[State::EscapeIntermediate as usize][0x7F] =
        Transition::new(State::EscapeIntermediate, ActionType::Ignore);

    // ========================
    // CsiEntry state
    // ========================
    // 0x00-0x17: execute
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x2F: collect → CsiIntermediate
    // 0x30-0x39: param → CsiParam
    // 0x3A (:): param → CsiParam (colon for SGR subparameters like 4:3)
    // 0x3B (;): param → CsiParam
    // 0x3C-0x3F: collect → CsiParam (private markers)
    // 0x40-0x7E: csi_dispatch → Ground
    // 0x7F: ignore
    set_range(
        &mut table,
        State::CsiEntry,
        0x00,
        0x17,
        Transition::new(State::CsiEntry, ActionType::Execute),
    );
    table[State::CsiEntry as usize][0x19] = Transition::new(State::CsiEntry, ActionType::Execute);
    set_range(
        &mut table,
        State::CsiEntry,
        0x1C,
        0x1F,
        Transition::new(State::CsiEntry, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::CsiEntry,
        0x20,
        0x2F,
        Transition::new(State::CsiIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::CsiEntry,
        0x30,
        0x39,
        Transition::new(State::CsiParam, ActionType::Param),
    );
    table[State::CsiEntry as usize][0x3A] = Transition::new(State::CsiParam, ActionType::Param);
    table[State::CsiEntry as usize][0x3B] = Transition::new(State::CsiParam, ActionType::Param);
    set_range(
        &mut table,
        State::CsiEntry,
        0x3C,
        0x3F,
        Transition::new(State::CsiParam, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::CsiEntry,
        0x40,
        0x7E,
        Transition::new(State::Ground, ActionType::CsiDispatch),
    );
    table[State::CsiEntry as usize][0x7F] = Transition::new(State::CsiEntry, ActionType::Ignore);

    // ========================
    // CsiParam state
    // ========================
    // 0x00-0x17: execute
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x2F: collect → CsiIntermediate
    // 0x30-0x39: param
    // 0x3A: param (colon for SGR subparameters)
    // 0x3B: param
    // 0x3C-0x3F: → CsiIgnore (invalid)
    // 0x40-0x7E: csi_dispatch → Ground
    // 0x7F: ignore
    set_range(
        &mut table,
        State::CsiParam,
        0x00,
        0x17,
        Transition::new(State::CsiParam, ActionType::Execute),
    );
    table[State::CsiParam as usize][0x19] = Transition::new(State::CsiParam, ActionType::Execute);
    set_range(
        &mut table,
        State::CsiParam,
        0x1C,
        0x1F,
        Transition::new(State::CsiParam, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::CsiParam,
        0x20,
        0x2F,
        Transition::new(State::CsiIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::CsiParam,
        0x30,
        0x39,
        Transition::new(State::CsiParam, ActionType::Param),
    );
    table[State::CsiParam as usize][0x3A] = Transition::new(State::CsiParam, ActionType::Param);
    table[State::CsiParam as usize][0x3B] = Transition::new(State::CsiParam, ActionType::Param);
    set_range(
        &mut table,
        State::CsiParam,
        0x3C,
        0x3F,
        Transition::new(State::CsiIgnore, ActionType::None),
    );
    set_range(
        &mut table,
        State::CsiParam,
        0x40,
        0x7E,
        Transition::new(State::Ground, ActionType::CsiDispatch),
    );
    table[State::CsiParam as usize][0x7F] = Transition::new(State::CsiParam, ActionType::Ignore);

    // ========================
    // CsiIntermediate state
    // ========================
    // 0x00-0x17: execute
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x2F: collect
    // 0x30-0x3F: → CsiIgnore
    // 0x40-0x7E: csi_dispatch → Ground
    // 0x7F: ignore
    set_range(
        &mut table,
        State::CsiIntermediate,
        0x00,
        0x17,
        Transition::new(State::CsiIntermediate, ActionType::Execute),
    );
    table[State::CsiIntermediate as usize][0x19] =
        Transition::new(State::CsiIntermediate, ActionType::Execute);
    set_range(
        &mut table,
        State::CsiIntermediate,
        0x1C,
        0x1F,
        Transition::new(State::CsiIntermediate, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::CsiIntermediate,
        0x20,
        0x2F,
        Transition::new(State::CsiIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::CsiIntermediate,
        0x30,
        0x3F,
        Transition::new(State::CsiIgnore, ActionType::None),
    );
    set_range(
        &mut table,
        State::CsiIntermediate,
        0x40,
        0x7E,
        Transition::new(State::Ground, ActionType::CsiDispatch),
    );
    table[State::CsiIntermediate as usize][0x7F] =
        Transition::new(State::CsiIntermediate, ActionType::Ignore);

    // ========================
    // CsiIgnore state
    // ========================
    // 0x00-0x17: execute
    // 0x19: execute
    // 0x1C-0x1F: execute
    // 0x20-0x3F: ignore
    // 0x40-0x7E: → Ground
    // 0x7F: ignore
    set_range(
        &mut table,
        State::CsiIgnore,
        0x00,
        0x17,
        Transition::new(State::CsiIgnore, ActionType::Execute),
    );
    table[State::CsiIgnore as usize][0x19] = Transition::new(State::CsiIgnore, ActionType::Execute);
    set_range(
        &mut table,
        State::CsiIgnore,
        0x1C,
        0x1F,
        Transition::new(State::CsiIgnore, ActionType::Execute),
    );
    set_range(
        &mut table,
        State::CsiIgnore,
        0x20,
        0x3F,
        Transition::new(State::CsiIgnore, ActionType::Ignore),
    );
    set_range(
        &mut table,
        State::CsiIgnore,
        0x40,
        0x7E,
        Transition::new(State::Ground, ActionType::None),
    );
    table[State::CsiIgnore as usize][0x7F] = Transition::new(State::CsiIgnore, ActionType::Ignore);

    // ========================
    // DcsEntry state
    // ========================
    // 0x00-0x17: ignore
    // 0x19: ignore
    // 0x1C-0x1F: ignore
    // 0x20-0x2F: collect → DcsIntermediate
    // 0x30-0x39: param → DcsParam
    // 0x3A: → DcsIgnore
    // 0x3B: param → DcsParam
    // 0x3C-0x3F: collect → DcsParam
    // 0x40-0x7E: dcs_hook → DcsPassthrough
    // 0x7F: ignore
    set_range(
        &mut table,
        State::DcsEntry,
        0x00,
        0x17,
        Transition::new(State::DcsEntry, ActionType::Ignore),
    );
    table[State::DcsEntry as usize][0x19] = Transition::new(State::DcsEntry, ActionType::Ignore);
    set_range(
        &mut table,
        State::DcsEntry,
        0x1C,
        0x1F,
        Transition::new(State::DcsEntry, ActionType::Ignore),
    );
    set_range(
        &mut table,
        State::DcsEntry,
        0x20,
        0x2F,
        Transition::new(State::DcsIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::DcsEntry,
        0x30,
        0x39,
        Transition::new(State::DcsParam, ActionType::Param),
    );
    table[State::DcsEntry as usize][0x3A] = Transition::new(State::DcsIgnore, ActionType::None);
    table[State::DcsEntry as usize][0x3B] = Transition::new(State::DcsParam, ActionType::Param);
    set_range(
        &mut table,
        State::DcsEntry,
        0x3C,
        0x3F,
        Transition::new(State::DcsParam, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::DcsEntry,
        0x40,
        0x7E,
        Transition::new(State::DcsPassthrough, ActionType::DcsHook),
    );
    table[State::DcsEntry as usize][0x7F] = Transition::new(State::DcsEntry, ActionType::Ignore);

    // ========================
    // DcsParam state
    // ========================
    // 0x00-0x17: ignore
    // 0x19: ignore
    // 0x1C-0x1F: ignore
    // 0x20-0x2F: collect → DcsIntermediate
    // 0x30-0x39: param
    // 0x3A: → DcsIgnore
    // 0x3B: param
    // 0x3C-0x3F: → DcsIgnore
    // 0x40-0x7E: dcs_hook → DcsPassthrough
    // 0x7F: ignore
    set_range(
        &mut table,
        State::DcsParam,
        0x00,
        0x17,
        Transition::new(State::DcsParam, ActionType::Ignore),
    );
    table[State::DcsParam as usize][0x19] = Transition::new(State::DcsParam, ActionType::Ignore);
    set_range(
        &mut table,
        State::DcsParam,
        0x1C,
        0x1F,
        Transition::new(State::DcsParam, ActionType::Ignore),
    );
    set_range(
        &mut table,
        State::DcsParam,
        0x20,
        0x2F,
        Transition::new(State::DcsIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::DcsParam,
        0x30,
        0x39,
        Transition::new(State::DcsParam, ActionType::Param),
    );
    table[State::DcsParam as usize][0x3A] = Transition::new(State::DcsIgnore, ActionType::None);
    table[State::DcsParam as usize][0x3B] = Transition::new(State::DcsParam, ActionType::Param);
    set_range(
        &mut table,
        State::DcsParam,
        0x3C,
        0x3F,
        Transition::new(State::DcsIgnore, ActionType::None),
    );
    set_range(
        &mut table,
        State::DcsParam,
        0x40,
        0x7E,
        Transition::new(State::DcsPassthrough, ActionType::DcsHook),
    );
    table[State::DcsParam as usize][0x7F] = Transition::new(State::DcsParam, ActionType::Ignore);

    // ========================
    // DcsIntermediate state
    // ========================
    // 0x00-0x17: ignore
    // 0x19: ignore
    // 0x1C-0x1F: ignore
    // 0x20-0x2F: collect
    // 0x30-0x3F: → DcsIgnore
    // 0x40-0x7E: dcs_hook → DcsPassthrough
    // 0x7F: ignore
    set_range(
        &mut table,
        State::DcsIntermediate,
        0x00,
        0x17,
        Transition::new(State::DcsIntermediate, ActionType::Ignore),
    );
    table[State::DcsIntermediate as usize][0x19] =
        Transition::new(State::DcsIntermediate, ActionType::Ignore);
    set_range(
        &mut table,
        State::DcsIntermediate,
        0x1C,
        0x1F,
        Transition::new(State::DcsIntermediate, ActionType::Ignore),
    );
    set_range(
        &mut table,
        State::DcsIntermediate,
        0x20,
        0x2F,
        Transition::new(State::DcsIntermediate, ActionType::Collect),
    );
    set_range(
        &mut table,
        State::DcsIntermediate,
        0x30,
        0x3F,
        Transition::new(State::DcsIgnore, ActionType::None),
    );
    set_range(
        &mut table,
        State::DcsIntermediate,
        0x40,
        0x7E,
        Transition::new(State::DcsPassthrough, ActionType::DcsHook),
    );
    table[State::DcsIntermediate as usize][0x7F] =
        Transition::new(State::DcsIntermediate, ActionType::Ignore);

    // ========================
    // DcsPassthrough state
    // ========================
    // 0x00-0x17: dcs_put
    // 0x19: dcs_put
    // 0x1C-0x1F: dcs_put
    // 0x20-0x7E: dcs_put
    // 0x7F: ignore
    // Note: 0x9C handled by ANYWHERE rules (dcs_unhook implicit)
    set_range(
        &mut table,
        State::DcsPassthrough,
        0x00,
        0x17,
        Transition::new(State::DcsPassthrough, ActionType::DcsPut),
    );
    table[State::DcsPassthrough as usize][0x19] =
        Transition::new(State::DcsPassthrough, ActionType::DcsPut);
    set_range(
        &mut table,
        State::DcsPassthrough,
        0x1C,
        0x1F,
        Transition::new(State::DcsPassthrough, ActionType::DcsPut),
    );
    set_range(
        &mut table,
        State::DcsPassthrough,
        0x20,
        0x7E,
        Transition::new(State::DcsPassthrough, ActionType::DcsPut),
    );
    table[State::DcsPassthrough as usize][0x7F] =
        Transition::new(State::DcsPassthrough, ActionType::Ignore);

    // ========================
    // DcsIgnore state
    // ========================
    // 0x00-0x17: ignore
    // 0x19: ignore
    // 0x1C-0x1F: ignore
    // 0x20-0x7F: ignore
    // Note: 0x9C handled by ANYWHERE rules
    set_range(
        &mut table,
        State::DcsIgnore,
        0x00,
        0x17,
        Transition::new(State::DcsIgnore, ActionType::Ignore),
    );
    table[State::DcsIgnore as usize][0x19] = Transition::new(State::DcsIgnore, ActionType::Ignore);
    set_range(
        &mut table,
        State::DcsIgnore,
        0x1C,
        0x1F,
        Transition::new(State::DcsIgnore, ActionType::Ignore),
    );
    set_range(
        &mut table,
        State::DcsIgnore,
        0x20,
        0x7F,
        Transition::new(State::DcsIgnore, ActionType::Ignore),
    );

    // ========================
    // OscString state
    // ========================
    // 0x00-0x06: ignore (or osc_put in some implementations)
    // 0x07: osc_end → Ground (BEL terminates OSC)
    // 0x08-0x17: ignore
    // 0x19: ignore
    // 0x1C-0x1F: ignore
    // 0x20-0xFF: osc_put (collect OSC data)
    // Note: ST (0x9C or ESC \) handled by ANYWHERE rules
    set_range(
        &mut table,
        State::OscString,
        0x00,
        0x06,
        Transition::new(State::OscString, ActionType::Ignore),
    );
    table[State::OscString as usize][0x07] = Transition::new(State::Ground, ActionType::OscEnd); // BEL
    set_range(
        &mut table,
        State::OscString,
        0x08,
        0x17,
        Transition::new(State::OscString, ActionType::Ignore),
    );
    table[State::OscString as usize][0x19] = Transition::new(State::OscString, ActionType::Ignore);
    set_range(
        &mut table,
        State::OscString,
        0x1C,
        0x1F,
        Transition::new(State::OscString, ActionType::Ignore),
    );
    set_range(
        &mut table,
        State::OscString,
        0x20,
        0x7F,
        Transition::new(State::OscString, ActionType::OscPut),
    );
    // High bytes also collect (for UTF-8 in OSC)
    set_range(
        &mut table,
        State::OscString,
        0xA0,
        0xFF,
        Transition::new(State::OscString, ActionType::OscPut),
    );

    // ========================
    // SosPmApcString state
    // ========================
    // For APC (Kitty graphics), we collect data bytes via ApcPut.
    // For SOS/PM, we ignore data but the action will be filtered by apc_active flag.
    // 0x00-0x17: ApcPut (collect for APC, ignored for SOS/PM by flag)
    // 0x19: ApcPut
    // 0x1C-0x1F: ApcPut
    // 0x20-0x7F: ApcPut
    // High bytes (0x80+): ApcPut (for UTF-8 in APC payloads)
    // Note: ST (0x9C or ESC \) handled by ANYWHERE rules
    set_range(
        &mut table,
        State::SosPmApcString,
        0x00,
        0x17,
        Transition::new(State::SosPmApcString, ActionType::ApcPut),
    );
    table[State::SosPmApcString as usize][0x19] =
        Transition::new(State::SosPmApcString, ActionType::ApcPut);
    set_range(
        &mut table,
        State::SosPmApcString,
        0x1C,
        0x1F,
        Transition::new(State::SosPmApcString, ActionType::ApcPut),
    );
    set_range(
        &mut table,
        State::SosPmApcString,
        0x20,
        0x7F,
        Transition::new(State::SosPmApcString, ActionType::ApcPut),
    );
    // High bytes also collect (for UTF-8 in APC payloads like Kitty graphics)
    set_range(
        &mut table,
        State::SosPmApcString,
        0xA0,
        0xFF,
        Transition::new(State::SosPmApcString, ActionType::ApcPut),
    );

    table
}

/// The compile-time generated transition table.
pub static TRANSITIONS: [[Transition; 256]; State::COUNT] = generate_table();
