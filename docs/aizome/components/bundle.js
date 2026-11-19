/* @ds-bundle: {"format":4,"namespace":"Aizome","components":[{"name":"Button"},{"name":"TextField"},{"name":"Select"},{"name":"Checkbox"},{"name":"RadioGroup"},{"name":"Switch"},{"name":"Badge"},{"name":"Alert"},{"name":"Card"},{"name":"Tabs"},{"name":"Table"},{"name":"Dialog"},{"name":"SideNav"},{"name":"Icon"}]} */
(function () {
  "use strict";
  var React = window.React;
  if (!React) { return; }
  var h = React.createElement;

  function cx() {
    var out = [];
    for (var i = 0; i < arguments.length; i++) { if (arguments[i]) out.push(arguments[i]); }
    return out.join(" ");
  }
  function omit(obj, keys) {
    var out = {};
    for (var k in obj) { if (Object.prototype.hasOwnProperty.call(obj, k) && keys.indexOf(k) < 0) out[k] = obj[k]; }
    return out;
  }
  var counter = 0;
  function useAutoId() {
    if (React.useId) return React.useId();
    var ref = React.useRef(null);
    if (ref.current === null) { counter += 1; ref.current = "az-" + counter; }
    return ref.current;
  }
  function useControllable(value, defaultValue, onChange) {
    var state = React.useState(defaultValue);
    var controlled = value !== undefined;
    var current = controlled ? value : state[0];
    var setState = state[1];
    var set = React.useCallback(function (next) {
      if (!controlled) setState(next);
      if (onChange) onChange(next);
    }, [controlled, onChange]);
    return [current, set];
  }

  /* ---------- Icon ---------- */
  var ICONS = {
    "check": [["path", { d: "M5 12.5l4.5 4.5L19 7.5" }]],
    "minus": [["path", { d: "M6 12h12" }]],
    "plus": [["path", { d: "M12 5v14M5 12h14" }]],
    "x": [["path", { d: "M6 6l12 12M18 6L6 18" }]],
    "chevron-down": [["path", { d: "M6 9l6 6 6-6" }]],
    "chevron-right": [["path", { d: "M9 6l6 6-6 6" }]],
    "chevron-left": [["path", { d: "M15 6l-6 6 6 6" }]],
    "arrow-right": [["path", { d: "M5 12h14M13 6l6 6-6 6" }]],
    "search": [["circle", { cx: 11, cy: 11, r: 6.5 }], ["path", { d: "M16 16l4 4" }]],
    "info": [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M12 11v5.5M12 7.6v.01" }]],
    "check-circle": [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M8.5 12.3l2.4 2.4 4.6-5" }]],
    "alert-triangle": [["path", { d: "M12 4.2L3 19.5h18z" }], ["path", { d: "M12 10v4.2M12 16.9v.01" }]],
    "alert-circle": [["circle", { cx: 12, cy: 12, r: 9 }], ["path", { d: "M12 7.5v5.2M12 16.2v.01" }]],
    "home": [["path", { d: "M4 11l8-6.5 8 6.5" }], ["path", { d: "M6 9.5V20h12V9.5" }]],
    "inbox": [["path", { d: "M4 13.5L6.5 5h11L20 13.5V19H4z" }], ["path", { d: "M4 13.5h4.5l1.2 2.2h4.6l1.2-2.2H20" }]],
    "users": [["circle", { cx: 9, cy: 8.5, r: 3.5 }], ["path", { d: "M3 19.5c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5M15.5 5.2a3.5 3.5 0 0 1 0 6.6M17.5 14.4c2 .7 3.5 2.5 3.5 5.1" }]],
    "chart": [["path", { d: "M4 4v16h16M8.5 16v-4M12.5 16V8M16.5 16v-6" }]],
    "file": [["path", { d: "M7 3.5h7l4 4v13H7z" }], ["path", { d: "M14 3.5V8h4M10 12.5h5M10 16h5" }]],
    "folder": [["path", { d: "M3.5 6.5h6l2 2h9v10h-17z" }]],
    "credit-card": [["rect", { x: 3, y: 5.5, width: 18, height: 13, rx: 2 }], ["path", { d: "M3 10h18M7 15h3" }]],
    "settings": [["path", { d: "M4 7h9M17 7h3M4 17h3M11 17h9" }], ["circle", { cx: 15, cy: 7, r: 2 }], ["circle", { cx: 9, cy: 17, r: 2 }]],
    "bell": [["path", { d: "M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15z" }], ["path", { d: "M10 21h4" }]],
    "download": [["path", { d: "M12 4v11M7 10.5l5 4.5 5-4.5M5 20h14" }]],
    "calendar": [["rect", { x: 4, y: 5.5, width: 16, height: 14.5, rx: 2 }], ["path", { d: "M4 10h16M8.5 3.5v4M15.5 3.5v4" }]],
    "more": [["circle", { cx: 6, cy: 12, r: 1.4, fill: "currentColor", stroke: "none" }], ["circle", { cx: 12, cy: 12, r: 1.4, fill: "currentColor", stroke: "none" }], ["circle", { cx: 18, cy: 12, r: 1.4, fill: "currentColor", stroke: "none" }]],
    "log-out": [["path", { d: "M10 4.5H5.5v15H10M14 8l4 4-4 4M18 12H9" }]]
  };
  function Icon(p) {
    var spec = ICONS[p.name];
    if (!spec) return null;
    var size = p.size || 20;
    var props = {
      viewBox: "0 0 24 24", width: size, height: size, fill: "none", stroke: "currentColor",
      strokeWidth: p.strokeWidth || 1.75, strokeLinecap: "round", strokeLinejoin: "round",
      className: cx("az-icon", p.className), focusable: "false"
    };
    if (p.label) { props.role = "img"; props["aria-label"] = p.label; } else { props["aria-hidden"] = true; }
    var args = ["svg", props];
    for (var i = 0; i < spec.length; i++) args.push(h(spec[i][0], spec[i][1]));
    return h.apply(null, args);
  }
  Icon.names = Object.keys(ICONS);
  function renderIcon(icon, size) {
    if (!icon) return null;
    if (typeof icon === "string") return h(Icon, { name: icon, size: size });
    return icon;
  }

  /* ---------- Button ---------- */
  function Button(p) {
    var variant = p.variant || "secondary";
    var size = p.size || "md";
    var iconSize = size === "sm" ? 16 : 18;
    var hasLabel = p.children !== undefined && p.children !== null && p.children !== false;
    var rest = omit(p, ["variant", "size", "iconStart", "iconEnd", "loading", "fullWidth", "className", "children", "type", "disabled"]);
    return h("button", Object.assign({ type: p.type || "button" }, rest, {
      className: cx("az-btn", "az-btn--" + variant, "az-btn--" + size, !hasLabel && "az-btn--icon", p.fullWidth && "az-btn--block", p.loading && "az-btn--loading", p.className),
      disabled: p.disabled || p.loading,
      "aria-busy": p.loading ? true : undefined
    }),
      p.loading ? h("span", { className: "az-spinner", "aria-hidden": true }) : renderIcon(p.iconStart, iconSize),
      hasLabel ? h("span", { className: "az-btn__label" }, p.children) : null,
      renderIcon(p.iconEnd, iconSize));
  }

  /* ---------- TextField ---------- */
  function TextField(p) {
    var autoId = useAutoId();
    var id = p.id || autoId;
    var descId = id + "-desc", errId = id + "-err";
    var describedBy = [p.description && !p.error ? descId : null, p.error ? errId : null].filter(Boolean).join(" ") || undefined;
    var rest = omit(p, ["label", "description", "error", "multiline", "rows", "size", "className", "id", "icon", "suffix", "required"]);
    var control = h(p.multiline ? "textarea" : "input", Object.assign({}, rest, {
      id: id, className: "az-input__control", rows: p.multiline ? (p.rows || 3) : undefined,
      required: p.required, "aria-invalid": p.error ? true : undefined, "aria-describedby": describedBy
    }));
    return h("div", { className: cx("az-field", p.className) },
      p.label ? h("label", { className: "az-field__label", htmlFor: id }, p.label, p.required ? h("span", { className: "az-field__req" }, "必須") : null) : null,
      h("div", { className: cx("az-input", "az-input--" + (p.size || "md"), p.multiline && "az-input--multiline", p.error && "az-input--invalid", p.disabled && "az-input--disabled") },
        p.icon ? h("span", { className: "az-input__affix" }, renderIcon(p.icon, 18)) : null,
        control,
        p.suffix ? h("span", { className: "az-input__affix" }, p.suffix) : null),
      p.description && !p.error ? h("p", { id: descId, className: "az-field__desc" }, p.description) : null,
      p.error ? h("p", { id: errId, className: "az-field__error" }, h(Icon, { name: "alert-circle", size: 14 }), h("span", null, p.error)) : null);
  }

  /* ---------- Select ---------- */
  function Select(p) {
    var autoId = useAutoId();
    var id = p.id || autoId;
    var descId = id + "-desc", errId = id + "-err";
    var describedBy = [p.description && !p.error ? descId : null, p.error ? errId : null].filter(Boolean).join(" ") || undefined;
    var rest = omit(p, ["label", "description", "error", "options", "placeholder", "size", "className", "id", "children", "required"]);
    if (p.placeholder && p.value === undefined && p.defaultValue === undefined) rest.defaultValue = "";
    var opts = (p.options || []).map(function (o) {
      var v = typeof o === "string" ? o : o.value;
      var l = typeof o === "string" ? o : o.label;
      return h("option", { key: v, value: v, disabled: o.disabled }, l);
    });
    if (p.placeholder) opts.unshift(h("option", { key: "__placeholder", value: "", disabled: true }, p.placeholder));
    return h("div", { className: cx("az-field", p.className) },
      p.label ? h("label", { className: "az-field__label", htmlFor: id }, p.label, p.required ? h("span", { className: "az-field__req" }, "必須") : null) : null,
      h("div", { className: cx("az-input", "az-select", "az-input--" + (p.size || "md"), p.error && "az-input--invalid", p.disabled && "az-input--disabled") },
        h("select", Object.assign({}, rest, { id: id, className: "az-input__control", required: p.required, "aria-invalid": p.error ? true : undefined, "aria-describedby": describedBy }), opts, p.children),
        h(Icon, { name: "chevron-down", size: 16, className: "az-select__chevron" })),
      p.description && !p.error ? h("p", { id: descId, className: "az-field__desc" }, p.description) : null,
      p.error ? h("p", { id: errId, className: "az-field__error" }, h(Icon, { name: "alert-circle", size: 14 }), h("span", null, p.error)) : null);
  }

  /* ---------- Checkbox ---------- */
  function Checkbox(p) {
    var autoId = useAutoId();
    var id = p.id || autoId;
    var ref = React.useRef(null);
    React.useEffect(function () { if (ref.current) ref.current.indeterminate = !!p.indeterminate; }, [p.indeterminate]);
    var rest = omit(p, ["label", "description", "indeterminate", "className", "id"]);
    return h("div", { className: cx("az-check", p.disabled && "is-disabled", p.className) },
      h("span", { className: "az-check__box" },
        h("input", Object.assign({}, rest, { ref: ref, id: id, type: "checkbox", className: "az-check__input", "aria-describedby": p.description ? id + "-desc" : undefined })),
        h(Icon, { name: p.indeterminate ? "minus" : "check", size: 14, strokeWidth: 2.5, className: "az-check__mark" })),
      (p.label || p.description) ? h("span", { className: "az-check__text" },
        p.label ? h("label", { htmlFor: id, className: "az-check__label" }, p.label) : null,
        p.description ? h("span", { id: id + "-desc", className: "az-check__desc" }, p.description) : null) : null);
  }

  /* ---------- RadioGroup ---------- */
  function RadioGroup(p) {
    var autoId = useAutoId();
    var name = p.name || autoId;
    var st = useControllable(p.value, p.defaultValue, p.onChange);
    return h("fieldset", { className: cx("az-radio-group", p.orientation === "horizontal" && "az-radio-group--h", p.className), disabled: p.disabled },
      p.label ? h("legend", { className: "az-field__label" }, p.label) : null,
      h("div", { className: "az-radio-group__items" }, (p.options || []).map(function (o) {
        var oid = name + "-" + o.value;
        return h("div", { key: o.value, className: cx("az-check", o.disabled && "is-disabled") },
          h("span", { className: "az-check__box" },
            h("input", { type: "radio", id: oid, name: name, value: o.value, checked: st[0] === o.value, disabled: o.disabled, className: "az-check__input az-radio__input", onChange: function () { st[1](o.value); }, "aria-describedby": o.description ? oid + "-desc" : undefined })),
          h("span", { className: "az-check__text" },
            h("label", { htmlFor: oid, className: "az-check__label" }, o.label),
            o.description ? h("span", { id: oid + "-desc", className: "az-check__desc" }, o.description) : null));
      })));
  }

  /* ---------- Switch ---------- */
  function Switch(p) {
    var autoId = useAutoId();
    var id = p.id || autoId;
    var st = useControllable(p.checked, !!p.defaultChecked, p.onChange);
    return h("div", { className: cx("az-switch", p.disabled && "is-disabled", p.className) },
      h("button", { type: "button", role: "switch", id: id, "aria-checked": !!st[0], disabled: p.disabled, className: "az-switch__track", onClick: function () { st[1](!st[0]); }, "aria-describedby": p.description ? id + "-desc" : undefined, "aria-label": p.label ? undefined : p["aria-label"] },
        h("span", { className: "az-switch__thumb" })),
      (p.label || p.description) ? h("span", { className: "az-check__text" },
        p.label ? h("label", { htmlFor: id, className: "az-check__label" }, p.label) : null,
        p.description ? h("span", { id: id + "-desc", className: "az-check__desc" }, p.description) : null) : null);
  }

  /* ---------- Badge ---------- */
  function Badge(p) {
    var tone = p.tone || "neutral";
    return h("span", { className: cx("az-badge", "az-badge--" + tone, p.className) },
      p.dot ? h("span", { className: "az-badge__dot", "aria-hidden": true }) : null,
      p.children);
  }

  /* ---------- Alert ---------- */
  var ALERT_ICON = { info: "info", success: "check-circle", warning: "alert-triangle", danger: "alert-circle" };
  function Alert(p) {
    var tone = p.tone || "info";
    return h("div", { className: cx("az-alert", "az-alert--" + tone, p.className), role: tone === "danger" || tone === "warning" ? "alert" : "status" },
      h(Icon, { name: ALERT_ICON[tone], size: 20, className: "az-alert__icon" }),
      h("div", { className: "az-alert__content" },
        p.title ? h("p", { className: "az-alert__title" }, p.title) : null,
        p.children ? h("div", { className: "az-alert__body" }, p.children) : null,
        p.action ? h("div", { className: "az-alert__action" }, p.action) : null),
      p.onClose ? h(Button, { variant: "ghost", size: "sm", iconStart: "x", "aria-label": "閉じる", className: "az-alert__close", onClick: p.onClose }) : null);
  }

  /* ---------- Card ---------- */
  function Card(p) {
    var interactive = !!p.onClick;
    var rest = omit(p, ["title", "description", "actions", "footer", "children", "padding", "className", "as"]);
    if (interactive) { rest.tabIndex = p.tabIndex !== undefined ? p.tabIndex : 0; rest.role = p.role || "button"; }
    return h(p.as || "section", Object.assign({}, rest, { className: cx("az-card", p.padding === "none" && "az-card--flush", interactive && "az-card--interactive", p.className) }),
      (p.title || p.actions) ? h("header", { className: "az-card__header" },
        h("div", null,
          p.title ? h("h3", { className: "az-card__title" }, p.title) : null,
          p.description ? h("p", { className: "az-card__desc" }, p.description) : null),
        p.actions ? h("div", { className: "az-card__actions" }, p.actions) : null) : null,
      p.children !== undefined ? h("div", { className: "az-card__body" }, p.children) : null,
      p.footer ? h("footer", { className: "az-card__footer" }, p.footer) : null);
  }

  /* ---------- Tabs ---------- */
  function Tabs(p) {
    var items = p.items || [];
    var base = useAutoId();
    var st = useControllable(p.value, p.defaultValue !== undefined ? p.defaultValue : (items[0] && items[0].id), p.onChange);
    var active = null;
    for (var i = 0; i < items.length; i++) if (items[i].id === st[0]) active = items[i];
    function onKeyDown(e) {
      if (["ArrowRight", "ArrowLeft", "Home", "End"].indexOf(e.key) < 0) return;
      var enabled = items.filter(function (t) { return !t.disabled; });
      var idx = enabled.findIndex(function (t) { return t.id === st[0]; });
      var next = e.key === "ArrowRight" ? enabled[(idx + 1) % enabled.length]
        : e.key === "ArrowLeft" ? enabled[(idx - 1 + enabled.length) % enabled.length]
        : e.key === "Home" ? enabled[0] : enabled[enabled.length - 1];
      if (!next) return;
      e.preventDefault();
      st[1](next.id);
      var el = e.currentTarget.querySelector('[data-tab-id="' + String(next.id).replace(/"/g, '\\"') + '"]');
      if (el) el.focus();
    }
    return h("div", { className: cx("az-tabs", p.className) },
      h("div", { role: "tablist", className: "az-tabs__list", "aria-label": p.label, onKeyDown: onKeyDown },
        items.map(function (t) {
          var sel = t.id === st[0];
          return h("button", { key: t.id, type: "button", role: "tab", id: base + "-tab-" + t.id, "data-tab-id": t.id, "aria-selected": sel, "aria-controls": t.content !== undefined ? base + "-panel-" + t.id : undefined, tabIndex: sel ? 0 : -1, disabled: t.disabled, className: "az-tabs__tab", onClick: function () { st[1](t.id); } },
            t.label,
            t.count !== undefined && t.count !== null ? h("span", { className: "az-tabs__count" }, t.count) : null);
        })),
      active && active.content !== undefined ? h("div", { role: "tabpanel", id: base + "-panel-" + active.id, "aria-labelledby": base + "-tab-" + active.id, className: "az-tabs__panel", tabIndex: 0 }, active.content) : null);
  }

  /* ---------- Table ---------- */
  function Table(p) {
    var cols = p.columns || [], rows = p.rows || [];
    var rowKey = p.rowKey || "id";
    return h("div", { className: cx("az-table-wrap", p.className), style: p.maxHeight ? { maxHeight: p.maxHeight, overflowY: "auto" } : undefined },
      h("table", { className: cx("az-table", p.density === "compact" && "az-table--compact") },
        p.caption ? h("caption", { className: "az-table__caption" }, p.caption) : null,
        h("thead", null, h("tr", null, cols.map(function (c) {
          return h("th", { key: c.key, scope: "col", className: c.numeric ? "az-num" : undefined, style: { width: c.width, textAlign: c.align } }, c.header);
        }))),
        h("tbody", null, rows.length ? rows.map(function (r, i) {
          var key = r[rowKey] !== undefined ? r[rowKey] : i;
          var selected = p.selectedKey !== undefined && p.selectedKey === key;
          return h("tr", { key: key, className: cx(selected && "is-selected", p.onRowClick && "is-clickable"), "aria-selected": p.selectedKey !== undefined ? selected : undefined, onClick: p.onRowClick ? function () { p.onRowClick(r); } : undefined },
            cols.map(function (c) {
              return h("td", { key: c.key, className: c.numeric ? "az-num" : undefined, style: c.align ? { textAlign: c.align } : undefined }, c.render ? c.render(r) : r[c.key]);
            }));
        }) : h("tr", null, h("td", { colSpan: cols.length || 1, className: "az-table__empty" }, p.emptyText || "データがありません")))));
  }

  /* ---------- Dialog ---------- */
  function Dialog(p) {
    var titleId = useAutoId();
    var descId = titleId + "-desc";
    var panel = React.useRef(null);
    var onClose = p.onClose;
    React.useEffect(function () {
      if (!p.open) return undefined;
      var prev = document.activeElement;
      if (panel.current) panel.current.focus();
      function onKey(e) { if (e.key === "Escape" && onClose && p.dismissible !== false) onClose(); }
      document.addEventListener("keydown", onKey);
      return function () {
        document.removeEventListener("keydown", onKey);
        if (prev && prev.focus) prev.focus();
      };
    }, [p.open, onClose, p.dismissible]);
    if (!p.open) return null;
    var node = h("div", { className: "az-dialog__overlay", onMouseDown: function (e) { if (e.target === e.currentTarget && onClose && p.dismissible !== false) onClose(); } },
      h("div", { ref: panel, tabIndex: -1, role: p.tone === "danger" ? "alertdialog" : "dialog", "aria-modal": true, "aria-labelledby": titleId, "aria-describedby": p.description ? descId : undefined, className: cx("az-dialog", "az-dialog--" + (p.size || "md"), p.className) },
        h("div", { className: "az-dialog__header" },
          h("h2", { id: titleId, className: "az-dialog__title" }, p.title),
          onClose && p.dismissible !== false ? h(Button, { variant: "ghost", size: "sm", iconStart: "x", "aria-label": "閉じる", className: "az-dialog__close", onClick: onClose }) : null),
        p.description ? h("p", { id: descId, className: "az-dialog__desc" }, p.description) : null,
        p.children ? h("div", { className: "az-dialog__body" }, p.children) : null,
        p.footer ? h("div", { className: "az-dialog__footer" }, p.footer) : null));
    var RD = window.ReactDOM;
    if (p.portal !== false && RD && RD.createPortal && typeof document !== "undefined") return RD.createPortal(node, document.body);
    return node;
  }

  /* ---------- SideNav ---------- */
  function SideNav(p) {
    var st = useControllable(p.activeId, p.defaultActiveId, p.onSelect);
    var sections = p.sections || [];
    return h("nav", { className: cx("az-sidenav", p.className), "aria-label": p.label || "メインメニュー" },
      p.header ? h("div", { className: "az-sidenav__header" }, p.header) : null,
      h("div", { className: "az-sidenav__body" }, sections.map(function (s, si) {
        return h("div", { key: s.label || si, className: "az-sidenav__section" },
          s.label ? h("p", { className: "az-sidenav__heading" }, s.label) : null,
          h("ul", { className: "az-sidenav__list" }, (s.items || []).map(function (it) {
            var active = it.id === st[0];
            return h("li", { key: it.id },
              h(it.href ? "a" : "button", {
                href: it.href, type: it.href ? undefined : "button",
                className: cx("az-sidenav__item", active && "is-active"),
                "aria-current": active ? "page" : undefined,
                onClick: function () { st[1](it.id); }
              },
                typeof it.icon === "string" ? h(Icon, { name: it.icon, size: 18, className: "az-sidenav__icon" }) : (it.icon || null),
                h("span", { className: "az-sidenav__label" }, it.label),
                it.badge !== undefined && it.badge !== null ? h("span", { className: "az-sidenav__badge" }, it.badge) : null));
          })));
      })),
      p.footer ? h("div", { className: "az-sidenav__footer" }, p.footer) : null);
  }

  var api = { Button: Button, TextField: TextField, Select: Select, Checkbox: Checkbox, RadioGroup: RadioGroup, Switch: Switch, Badge: Badge, Alert: Alert, Card: Card, Tabs: Tabs, Table: Table, Dialog: Dialog, SideNav: SideNav, Icon: Icon };
  window.Aizome = Object.assign(window.Aizome || {}, api);
})();
