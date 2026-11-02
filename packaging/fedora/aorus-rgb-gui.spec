# o binário já sai sem símbolos (strip no Cargo.toml): não há debuginfo
%global debug_package %{nil}

Name:           aorus-rgb-gui
Version:        2.0.0
Release:        1%{?dist}
Summary:        GTK interface for aorus-rgb
# o código é GPL; as crates Rust embutidas no binário são MIT ou Apache-2.0
License:        GPL-2.0-or-later AND (MIT OR Apache-2.0) AND MIT
URL:            https://github.com/augustcaio/aorus-rgb
Source0:        %{url}/archive/v%{version}/aorus-rgb-%{version}.tar.gz
ExclusiveArch:  x86_64

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gtk4-devel
BuildRequires:  libadwaita-devel
BuildRequires:  desktop-file-utils
# a interface conversa com o daemon pelo D-Bus
Requires:       aorus-rgb >= %{version}
Requires:       hicolor-icon-theme

%description
AORUS RGB is the GTK 4 interface of aorus-rgb, laid out like
RGB Fusion 2.0. It sets effects and colors for every device at once or per
zone, on a drawing of the motherboard and of the memory sticks.

%prep
%autosetup -n aorus-rgb-%{version}

%build
cd gui
# shortcut: baixa as crates no build (precisa de rede); vendorizar para builds offline (Koji)
export CARGO_TARGET_DIR=target
cargo build --release --locked

%check
cd gui
export CARGO_TARGET_DIR=target
cargo test --release --locked

%install
install -Dpm755 gui/target/release/aorus-rgb-gui %{buildroot}%{_bindir}/aorus-rgb-gui
desktop-file-install --dir=%{buildroot}%{_datadir}/applications \
    gui/data/io.github.augustcaio.AorusRGB.Gui.desktop
install -Dpm644 gui/data/icons/hicolor/scalable/apps/io.github.augustcaio.AorusRGB.Gui.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.augustcaio.AorusRGB.Gui.svg
install -Dpm644 gui/data/icons/hicolor/symbolic/apps/io.github.augustcaio.AorusRGB.Gui-symbolic.svg \
    %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps/io.github.augustcaio.AorusRGB.Gui-symbolic.svg

%files
%{_bindir}/aorus-rgb-gui
%{_datadir}/applications/io.github.augustcaio.AorusRGB.Gui.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.augustcaio.AorusRGB.Gui.svg
%{_datadir}/icons/hicolor/symbolic/apps/io.github.augustcaio.AorusRGB.Gui-symbolic.svg

%changelog
* Mon Nov 02 2026 Caio Augusto <21364833+augustcaio@users.noreply.github.com> - 2.0.0-1
- Primeiro pacote para o Fedora
