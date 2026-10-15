#!/usr/bin/env bash
# Guard the stock-Adwaita rules for spacepods-ui (see docs/audit/01-gnome-hig-adwaita-assessment.md).
# Fails when the UI reintroduces custom styling or APIs that were removed on purpose.
set -u
SRC="${1:-spacepods-ui/src}"
fail=0

check() {
  local pattern="$1" why="$2"
  local hits
  hits=$(grep -rn --include=*.rs -E "$pattern" "$SRC" || true)
  if [ -n "$hits" ]; then
    echo "error: $why" >&2
    echo "$hits" >&2
    fail=1
  fi
}

# No custom stylesheets: the look comes from the desktop and libadwaita only.
check 'CssProvider|load_from_data|load_from_string|add_provider_for_display' \
  "custom CSS is not allowed; use stock libadwaita widgets and style classes"

# No colour literals: colours come from the theme (widget.color(), AdwStyleManager).
check 'set_source_rgba?\(\s*[0-9]' \
  "hard-coded colour literal in drawing code; read the theme colour instead"
check 'Rgba::new\(|RGBA::new\(' \
  "hard-coded RGBA value; read the theme colour instead"

# Deprecated GTK APIs that have Adwaita replacements.
check 'gtk4::Shortcuts(Window|Section|Group|Shortcut)|ShortcutsWindow::builder' \
  "GtkShortcutsWindow is deprecated; use libadwaita::ShortcutsDialog"
check '\.style_context\(\)' \
  "style_context() is deprecated since GTK 4.10; use widget.color() or libadwaita APIs"
check '\.show\(\)' \
  "WidgetExt::show is deprecated since GTK 4.10; use present() for windows or set_visible()"

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "ui style check passed ($SRC)"
