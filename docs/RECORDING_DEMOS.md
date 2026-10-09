# Recording demos for the README

The README should use short, honest demonstrations of what currently works. Keep the emulator demo and synthetic X11 test separate so viewers do not mistake either one for the unfinished LXC Android runtime.

## Recommended clips

### 1. APK workflow on the development emulator

- **Length:** about 20–35 seconds.
- Show the development emulator, launch 2048, and make a few moves.
- Optional: briefly show the CLI command that installed/launched the APK, but avoid leaving private paths, account names, or device details on screen.
- Caption it as **“2048 running in the Android SDK development emulator via the project CLI.”** Do not call it an LXC or native-window Android launch.

### 2. Native X11 display test

- **Length:** about 8–15 seconds.
- Run `./scripts/test-x11-display.sh` and show the smooth gradient window.
- Caption it as **“Synthetic-frame test of the host X11 display bridge.”** Do not describe the gradient as Android-rendered output.

## Capture tips

OBS Studio is a suitable screen recorder on Linux. Use a 1920×1080 canvas/output at 30 fps if practical, with H.264 MP4 for broad playback compatibility. A smaller window crop is fine if it remains readable. Trim dead time at the beginning and end.

Before recording, close private messages and unrelated windows, hide notifications, and avoid showing home-directory paths, credentials, private device identifiers, or unrelated personal information. Do not record passwords or access codes. Keep narration optional; short on-screen captions can explain the test's scope.

## Adding clips to the repository

Keep the original MP4s outside the source repository until the size and licensing implications are reviewed. A long or high-bitrate screen recording makes cloning the repository slower. Preferred options:

1. Host the full MP4 on a public video service or attach it to a GitHub release/discussion, then link to it from the README.
2. Put a short, optimized animated GIF in `assets/demo/` for an inline preview, and link the GIF to the full video.
3. Store the original locally or as a release asset rather than committing a large video binary to the main Git history.

GitHub README Markdown does not provide a reliable inline MP4 player. An animated GIF is the simplest inline preview; a thumbnail can link to the full video. Once media is ready, replace the “Demos” text in `README.md` with a short section like this:

```markdown
### APK workflow on the development emulator

[![Watch the 2048 emulator demo](assets/demo/2048-preview.gif)](FULL_VIDEO_URL)

### Native X11 display bridge

[![Watch the synthetic-gradient test](assets/demo/x11-gradient-preview.gif)](FULL_VIDEO_URL)
```

Replace `FULL_VIDEO_URL` with the public video/release URL. Keep the captions explicit about whether the frame is synthetic or Android-rendered.

## Optional: create an optimized GIF with FFmpeg

If FFmpeg is installed, this example converts a short excerpt into a loopable preview. Adjust the input path and duration to fit the clip:

```bash
ffmpeg -ss 00:00:02 -t 8 -i input.mp4 \
  -vf "fps=10,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer" \
  -loop 0 assets/demo/x11-gradient-preview.gif
```

Review the output for readability and file size before committing. Do not include videos of real Android output until that feature has actually been verified.
