NAME = cosmic-applet-hermes
APPID = io.github.hermes.CosmicAppletHermes
PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
DATDIR = $(PREFIX)/share
CARGO_TARGET_DIR ?= target

.PHONY: all build install uninstall clean

all: build

build:
	cargo build --release

install: build
	install -Dm0755 $(CARGO_TARGET_DIR)/release/$(NAME) $(BINDIR)/$(NAME)
	install -Dm0644 data/$(APPID).desktop $(DATDIR)/applications/$(APPID).desktop
	install -Dm0644 data/icons/hicolor/scalable/apps/$(APPID)-symbolic.svg $(DATDIR)/icons/hicolor/scalable/apps/$(APPID)-symbolic.svg

uninstall:
	rm -f $(BINDIR)/$(NAME)
	rm -f $(DATDIR)/applications/$(APPID).desktop
	rm -f $(DATDIR)/icons/hicolor/scalable/apps/$(APPID)-symbolic.svg

clean:
	cargo clean
