//
//  DashTerm-Bridging-Header.h
//  DashTerm
//
//  Bridging header for Rust FFI integration
//

#ifndef DashTerm_Bridging_Header_h
#define DashTerm_Bridging_Header_h

#include <stdint.h>
#include <stdbool.h>

// Include generated Rust FFI header
#include "dashterm.h"

// PTY functions (from Darwin)
#include <util.h>      // for openpty
#include <termios.h>   // for terminal control
#include <sys/ioctl.h> // for ioctl

#endif /* DashTerm_Bridging_Header_h */
