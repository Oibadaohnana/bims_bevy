{ pkgs ? import <nixpkgs> { } }:

let
  # What winit and wgpu dlopen at runtime: the window system and the GPU
  # loader. None of it is linked at build time, so a `cargo build` outside
  # this shell still works — it is `cargo run` that wants LD_LIBRARY_PATH.
  runtimeLibs = with pkgs; [
    vulkan-loader
    libxkbcommon
    wayland
    libx11
    libxcursor
    libxi
    libxrandr
  ];
in
pkgs.mkShell {
  name = "bims";
  packages = with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    pkg-config
    # The sounds are cut from the recordings by crates/app/sounds/prepare.sh.
    ffmpeg
    # The headless compositor `./hidden` opens a smoke run's window on, so
    # a run with nobody at the keyboard shows up on nobody's desktop.
    weston
  ];

  # ALSA is the one thing the sound *links*: cpal, under Bevy's audio,
  # finds it through pkg-config at build time, so a `cargo build` wants it
  # on the path as well as `cargo run`.
  buildInputs = runtimeLibs ++ [ pkgs.alsa-lib ];

  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
}
