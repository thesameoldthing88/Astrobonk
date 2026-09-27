#!/bin/bash
# One-time system setup for a Linux cloud container (Ubuntu): the libraries Bevy links against,
# plus a virtual display and a software Vulkan driver so the WINDOWED game can run headlessly
# for screenshots and two-instance co-op tests. Safe to re-run.
set -e
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev \
  libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 xvfb mesa-vulkan-drivers libvulkan1 \
  libgl1-mesa-dri libegl1 x11-utils xdotool imagemagick binutils >/dev/null
echo "setup done"
