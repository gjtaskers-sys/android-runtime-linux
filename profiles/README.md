# Runtime profiles

These are initial policy targets, not hard requirements.

## Lite
- Android: x86_64
- resolution: 1280x720-class
- refresh: 30 fps target on low-end systems
- 1-2 Android vCPUs worth of CPU scheduling
- lazy start/stop

## Balanced
- Android: x86_64
- resolution: 1600x900 or host-appropriate
- refresh: 60 fps target
- 2-4 CPU threads

## Compatibility
- Android: ARM64 VM backend
- used only when Native Bridge is insufficient
