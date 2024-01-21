# Declare the default target
.DEFAULT_GOAL := run

# Define the targets
flatpak:
	mkdir -p repo
	flatpak-builder --repo=repo --force-clean build-dir io.github.dvlv.boxbuddyrs.json
	flatpak build-bundle repo boxbuddy.flatpak io.github.dvlv.boxbuddyrs

run:
	cargo run

lint:
	cargo fmt
	cargo clippy

potfile:
	xtr src/main.rs -o po/boxbuddy.pot

# Declare a phony target for clean
.PHONY: clean

clean:
	rm -rf build-dir/*
	rm -rf target/*
	rm -rf .flatpak-builder/*

clean-flatpak:
	rm -rf build-dir/*
	rm -rf .flatpak-builder/*

