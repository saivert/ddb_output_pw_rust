# ddb_output_pw_rust
Pipewire output plugin for DeadBeef written in Rust

This uses my own fork of [pipewire-rs](https://gitlab.freedesktop.org/saivert/pipewire-rs) (hosted on gitlab) as the upstream version is far from complete yet and it is going to be a while until it gets a stable API.

I have added stuff to the stream API that is required by this plugin.

## Notes
This plugin is just a proof of concept and a starting point for others who wish to write a plugin for DeadBeef in the rust language. The version of this plugin that is written in C is going to remain the official one.

