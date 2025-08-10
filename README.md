## lazyserial

A fast, minimal TUI serial terminal inspired by lazygit. Built with ratatui + crossterm.

### Features
- Port discovery and selection
- Open/close with chosen baud rate
- Live output view with scrolling
- Input line to send text (newline appended)
- Lightweight, single binary

### Getting started
Requirements: Rust 1.73+ (stable), a serial device.

Build and run:
```sh
cargo run
```

On Linux you may need permissions for serial devices (e.g., add your user to `dialout` or adjust udev rules).

### Key bindings
- q: Quit
- Tab / Shift-Tab: Cycle focus (Ports → Output → Input)
- r: Refresh ports
- b / B: Cycle common baud rates forward/back
- Enter (Ports): Open/close selected port
- Enter (Input): Send current line (appends \n)
- PageUp/PageDown/Home/End (Output): Scroll

### Notes
- Default baud: 115200. Cycling order: 9600, 19200, 38400, 57600, 115200, 230400.
- Output pane shows sent lines prefixed with `>>`.
- Hex view and file logging
- Help popup and theming


