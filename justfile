name := 'cosmic-applet-hermes'
appid := 'io.github.hermes.CosmicAppletHermes'

prefix := '/usr/local'
bindir := prefix / 'bin'
datdir := prefix / 'share'

cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name

default: build

build:
    cargo build --release

install: build
    install -Dm0755 {{bin-src}} {{bindir}}/{{name}}
    install -Dm0644 data/{{appid}}.desktop {{datdir}}/applications/{{appid}}.desktop
    install -Dm0644 data/icons/hicolor/scalable/apps/{{appid}}-symbolic.svg {{datdir}}/icons/hicolor/scalable/apps/{{appid}}-symbolic.svg

uninstall:
    rm -f {{bindir}}/{{name}}
    rm -f {{datdir}}/applications/{{appid}}.desktop
    rm -f {{datdir}}/icons/hicolor/scalable/apps/{{appid}}-symbolic.svg

clean:
    cargo clean
