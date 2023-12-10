#!/bin/bash
flatpak-builder --repo=repo --force-clean build-dir io.github.dvlv.boxbuddyrs.json
flatpak build-bundle repo boxbuddy.flatpak io.github.dvlv.boxbuddyrs