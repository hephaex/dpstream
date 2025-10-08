#---------------------------------------------------------------------------------
# Makefile for dpstream Switch homebrew
#---------------------------------------------------------------------------------

#---------------------------------------------------------------------------------
# Application info
#---------------------------------------------------------------------------------
APP_TITLE   := dpstream
APP_AUTHOR  := Mario Cho
APP_VERSION := 1.0.0

#---------------------------------------------------------------------------------
# Directory structure
#---------------------------------------------------------------------------------
TARGET      := dpstream-switch
BUILD       := build
SOURCES     := src
INCLUDES    := include
ROMFS       := romfs

#---------------------------------------------------------------------------------
# Compiler and build options
#---------------------------------------------------------------------------------
ARCH        := -march=armv8-a+crc+crypto -mtune=cortex-a57 -mtp=soft -fPIE
CFLAGS      := -g -Wall -O2 -ffunction-sections $(ARCH) $(DEFINES)
CPPFLAGS    := $(CFLAGS) -fno-rtti -fno-exceptions -std=gnu++17
ASFLAGS     := -g $(ARCH)
LDFLAGS     := -specs=$(DEVKITPRO)/libnx/switch.specs -g $(ARCH) -Wl,-Map,$(notdir $*.map)

#---------------------------------------------------------------------------------
# Libraries and includes
#---------------------------------------------------------------------------------
LIBS        := -lnx
LIBDIRS     := $(PORTLIBS) $(LIBNX)

#---------------------------------------------------------------------------------
# Build rules
#---------------------------------------------------------------------------------
.PHONY: clean all

all: $(TARGET).nro

$(TARGET).nro: $(TARGET).elf $(TARGET).nacp
	elf2nro $< $@ --icon=icon.jpg --nacp=$(TARGET).nacp

$(TARGET).nacp:
	nacptool --create "$(APP_TITLE)" "$(APP_AUTHOR)" "$(APP_VERSION)" $@

$(TARGET).elf: target/aarch64-nintendo-switch-freestanding/release/$(TARGET)
	cp $< $@

# Build with cargo
target/aarch64-nintendo-switch-freestanding/release/$(TARGET):
	cargo build --release --target aarch64-nintendo-switch-freestanding

clean:
	@echo Cleaning...
	@rm -fr $(BUILD) target *.elf *.nro *.nacp

#---------------------------------------------------------------------------------
# Development helpers
#---------------------------------------------------------------------------------
run: $(TARGET).nro
	@echo "Copy $(TARGET).nro to /switch/ on your SD card"

install: $(TARGET).nro
	@if [ -d "/Volumes/Nintendo Switch" ]; then \
		mkdir -p "/Volumes/Nintendo Switch/switch"; \
		cp $(TARGET).nro "/Volumes/Nintendo Switch/switch/"; \
		echo "Installed to SD card"; \
	else \
		echo "SD card not found - copy $(TARGET).nro manually"; \
	fi

# For debugging with nxlink
debug: $(TARGET).nro
	nxlink -s $(TARGET).nro

#---------------------------------------------------------------------------------
# Help
#---------------------------------------------------------------------------------
help:
	@echo "dpstream Switch Homebrew Build System"
	@echo ""
	@echo "Targets:"
	@echo "  all      - Build NRO file"
	@echo "  clean    - Clean build files"
	@echo "  install  - Install to SD card (if mounted)"
	@echo "  debug    - Send via nxlink for debugging"
	@echo "  help     - Show this help"
	@echo ""
	@echo "Requirements:"
	@echo "  - devkitPro with devkitA64"
	@echo "  - Rust with aarch64-nintendo-switch-freestanding target"
	@echo "  - libnx"
	@echo ""