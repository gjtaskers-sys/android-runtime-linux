# Project page assets

This directory contains content intended to help present the repository clearly to newcomers.

## Logo

- `../assets/brand/android-runtime-logo.png` is the transparent project mark used in `README.md`.
- `../assets/brand/github-social-preview.png` is a 1280 × 640 repository preview graphic.

To set the social preview on GitHub, open the repository's **Settings → General → Social preview**, upload `assets/brand/github-social-preview.png`, and save. If the field is hidden, check that you have repository admin access.

## Demo clips

Follow [`RECORDING_DEMOS.md`](RECORDING_DEMOS.md) before recording or publishing a clip. Current demos need to distinguish the Android SDK emulator from the synthetic X11 gradient test. Do not present either as real Android graphics running in LXC.

## README maintenance

Keep the opening screen focused: project purpose, CI/license/status badges, what works, the most important limitation, and a short route to build/test/contribute. Move detailed setup and architecture content into the linked docs instead of making the README an exhaustive manual.
