Name:           aorus-rgb
Version:        2.0.2
Release:        1%{?dist}
Summary:        RGB lighting control for the Gigabyte B550M AORUS ELITE and ENE memory
License:        GPL-2.0-or-later
URL:            https://github.com/augustcaio/aorus-rgb
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz
BuildArch:      noarch

BuildRequires:  python3-devel
BuildRequires:  systemd-rpm-macros
# testes: o daemon pelo D-Bus num barramento privado (Gio.TestDBus)
BuildRequires:  python3-pytest
BuildRequires:  python3dist(pygobject)
BuildRequires:  dbus-daemon
Requires:       python3dist(pygobject)
Recommends:     aorus-rgb-gui
Suggests:       i2c-tools

%description
Daemon and command line tool for the LEDs of the Gigabyte B550M AORUS ELITE
(ITE IT5702 controller, RGB Fusion 2.0) and of RGB memory sticks with an ENE
controller. The daemon is the only process that talks to the hardware and
serves the command line tool and the GTK interface over the session bus.

%prep
%autosetup -n %{name}-%{version}

%generate_buildrequires
%pyproject_buildrequires

%build
%pyproject_wheel

%install
%pyproject_install
%pyproject_save_files -L aorus_rgb
install -Dpm644 70-aorus-rgb.rules %{buildroot}%{_udevrulesdir}/70-aorus-rgb.rules
install -Dpm644 aorus-rgb.service %{buildroot}%{_userunitdir}/aorus-rgb.service
install -Dpm644 packaging/modules-load.conf %{buildroot}%{_modulesloaddir}/aorus-rgb.conf
install -Dpm644 packaging/dbus/io.github.augustcaio.AorusRGB.service \
    %{buildroot}%{_datadir}/dbus-1/services/io.github.augustcaio.AorusRGB.service

%check
%pytest

%post
%udev_rules_update
# instalado depois do boot: aplica a regra aos dispositivos já presentes, sem
# reiniciar (o reload do udev pelo file trigger só vem no fim da transação)
udevadm control --reload 2>/dev/null || :
udevadm trigger --action=change --subsystem-match=hidraw --subsystem-match=i2c-dev 2>/dev/null || :
%systemd_user_post aorus-rgb.service

%preun
%systemd_user_preun aorus-rgb.service

%files -f %{pyproject_files}
%{_bindir}/aorus-rgb
%{_udevrulesdir}/70-aorus-rgb.rules
%{_userunitdir}/aorus-rgb.service
%{_modulesloaddir}/aorus-rgb.conf
%{_datadir}/dbus-1/services/io.github.augustcaio.AorusRGB.service

%changelog
* Tue Nov 03 2026 Caio Augusto <21364833+augustcaio@users.noreply.github.com> - 2.0.2-1
- Versão 2.0.2: RPM gerado no Fedora 45
- count salva a quantidade de LEDs, com ou sem o daemon (#1)
- A instalação aplica a regra udev aos dispositivos já presentes, sem reiniciar

* Mon Nov 02 2026 Caio Augusto <21364833+augustcaio@users.noreply.github.com> - 2.0.1-1
- Versão 2.0.1: primeira publicada com pacotes para o Fedora

* Mon Nov 02 2026 Caio Augusto <21364833+augustcaio@users.noreply.github.com> - 2.0.0-1
- Primeiro pacote para o Fedora
