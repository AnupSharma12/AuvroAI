.PHONY: deps build run build-release build-windows build-all package-linux package-windows appimage benchmark clean install-linux

ifeq ($(OS),Windows_NT)
WINDOWS_BUILD_CMD := cargo build --release
WINDOWS_RELEASE_DIR := target/release
BUILD_ALL_CMD := $(WINDOWS_BUILD_CMD)
else
WINDOWS_BUILD_CMD := cargo build --release --target x86_64-pc-windows-gnu
WINDOWS_RELEASE_DIR := target/x86_64-pc-windows-gnu/release
BUILD_ALL_CMD := bash scripts/build-all.sh
endif

deps:
	sudo apt install -y \
	  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
	  libxkbcommon-dev libssl-dev pkg-config libgtk-3-dev \
	  gcc-mingw-w64-x86-64 icnsutils zip wget

build:
	cargo build

run:
	cargo run

build-release:
	cargo build --release
	@echo "Binary: target/release/auvro_ai"
	@ls -lh target/release/auvro_ai

build-windows:
	$(WINDOWS_BUILD_CMD)

build-all:
	$(BUILD_ALL_CMD)

package-linux: build-release
	mkdir -p dist/linux
	cp target/release/auvro_ai dist/linux/
	cp assets/icon.png dist/linux/
	cp assets/auvroai.desktop dist/linux/
	tar -czf dist/AuvroAI-linux.tar.gz -C dist/linux .

package-windows: build-windows
	mkdir -p dist/windows
	cp $(WINDOWS_RELEASE_DIR)/auvro_ai.exe dist/windows/AuvroAI.exe
	cp assets/icon.ico dist/windows/

appimage:
	bash scripts/build-appimage.sh

benchmark:
	cargo build
	cargo build --release
	cargo run --bin benchmark

clean:
	cargo clean
	rm -rf dist/

install-linux: build-release
	sudo cp target/release/auvro_ai /usr/local/bin/auvro_ai
	sudo cp assets/icon.png /usr/share/pixmaps/auvroai.png
	sudo cp assets/auvroai.desktop /usr/share/applications/auvroai.desktop
	sudo update-desktop-database /usr/share/applications/
	@echo "Installed. Launch with: auvro_ai"
