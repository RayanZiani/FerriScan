# Accessibility checklist (release gate)

Run once per release on each platform. Record the build SHA and the tester.

## Automated

- [ ] Playwright + axe-core on every route: zero `serious` or `critical` violations.
- [ ] Token contrast assertion: text on background ≥ 4.5:1, large text ≥ 3:1, focus ring ≥ 3:1
      against adjacent colors, in light, dark and high contrast.
- [ ] Every `<button>`/`<a>`/input has an accessible name (`aria-label` or visible text).
- [ ] No positive `tabindex`; focus order follows reading order.

## Keyboard only (unplug the mouse)

- [ ] Launch → volumes list reachable with Tab.
- [ ] Start a scan with the keyboard; progress announced; cancel reachable.
- [ ] Tree table: arrows navigate, Left/Right collapse/expand, Home/End, PageUp/PageDown.
- [ ] Sort and filter without a pointer.
- [ ] Treemap/sunburst: arrow-key grid navigation, Enter to re-root, Backspace to go up.
- [ ] Select multiple with Shift+Space / Ctrl+Space.
- [ ] Open the action dialog, review the plan, confirm and cancel with the keyboard.
- [ ] Settings: every control operable, language and theme changes announced.
- [ ] Esc closes overlays and never leaves focus lost.
- [ ] No keyboard trap in any modal or menu.

## Screen reader

- [ ] NVDA or Narrator (Windows, native GPUI via AccessKit; unverified): names, roles, states of the volume list, tree rows,
      chart alternatives and dialogs are announced correctly.
- [ ] VoiceOver (macOS): same journey.
- [ ] Orca (Linux): same journey.
- [ ] `aria-live` announces scan start, progress milestones (not every tick), completion,
      action results and errors.
- [ ] Chart "Accessible data" table exposes the same numbers as the canvas.

## Visual and motion

- [ ] 200 % zoom: no clipping, no horizontal scroll on primary actions.
- [ ] High contrast mode: chart encoding uses patterns/borders, not color alone.
- [ ] Reduced motion: no transitions over 100 ms, no chart animation, no progress shimmer.
- [ ] Windows accent color failing AA is auto-corrected (spot-check a saturated accent).
- [ ] Text remains readable at 125 %, 150 %, 200 % Windows scaling.

## Content

- [ ] Error messages state what happened and what to do next.
- [ ] Destructive confirmations are unambiguous and name the target.
- [ ] No icon-only button without a tooltip and accessible name.

## Sign-off

| Platform | Build SHA | Tester | Date | Result |
| --- | --- | --- | --- | --- |
| Windows 11 | | | | |
| Ubuntu 24.04 | | | | |
| macOS 15 | | | | |
