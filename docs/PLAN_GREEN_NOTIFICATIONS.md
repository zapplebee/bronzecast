# Plan: Restore green-box notifications

Status: Implemented and verified with an encoded live frame
Scope: notification rendering only; can ship independently of sticker unification
Related: [Unify stickers](PLAN_UNIFY_STICKERS.md), [README](../README.md)

## Problem

Notifications currently use the sticker visual language: pastel peach/lemon/mint
pills, thick white die-cut borders, offset shadows, and mini fruit icons. That
makes ephemeral notifications look like persistent stickers and blurs the
distinction between the two domains.

The original dashboard identity was a dark green box: near-black fill, thin muted
green border, a 3px green left accent, green title, pale green body, and a muted
age label. The user wants that notification identity restored while the fruit
stickers keep their current look.

## Goals

- Restore the green-box notification cards.
- Remove fruit icons, pastel fills, white sticker borders, and sticker shadows
  from notifications.
- Keep notifications ephemeral and in-memory.
- Keep the notification API, limits, ordering, age labels, and count unchanged.
- Keep the header, clock, status metrics, character, and sticker layer unchanged;
  the open-item panel is outside this plan.
- Keep the fruit sticker layer visually distinct from notifications.

## Non-goals

- Do not change sticker visuals or sticker behavior.
- Do not make notifications persistent.
- Do not change notification validation or MCP tools.
- Do not change the clock, status metrics, character header, or sticker layer.
- Do not change the open item; its removal is covered by the sticker plan.
- Do not add browser overlays.

## Target visual specification

Notification cards return to the pre-sticker design:

| Property | Value |
| --- | --- |
| Card position | `x = 470`, `y = 267 + index * 82` |
| Card size | `460 × 75` |
| Fill | `rgba(4, 12, 7, 245)` (near-black green ink) |
| Border | 1px `rgba(81, 113, 66, 255)` (muted green) |
| Left accent | 3px wide `rgba(157, 208, 104, 255)` (green) |
| Title | 17px green, `x = 486`, baseline `y + 22`, wrap width 340, max 1 line |
| Age | 13px muted green, right side, baseline `y + 22` |
| Body | 16px `rgba(183, 198, 165, 255)` (pale green), `x = 486`, baseline `y + 43 + line * 18`, wrap width 428, max 2 lines |
| Empty state | `All quiet. Your host is on watch.` at `(476, 308)`, 20px ivory; `POST /api/notifications` at `(476, 337)`, 16px muted |

Keep the existing `NOTIFICATIONS / NEWEST FIRST` label at `(476, 253)`, 13px
green. Keep the current character header, clock, dashboard, and sticker draw
order.

The current word-aware wrapping in `src/notifications.rs` is better than the old
character-by-character wrapper. Keep the word-aware wrapper, but use the old card
geometry and widths.

## Implementation steps

1. Edit `src/notifications.rs`; remove the now-unused notification-only
   `mini_fruit` helper from `src/sticker_art.rs` without changing sticker drawing.
2. Delete the `cards` peach/lemon/mint palette.
3. Delete the empty-state `sticker_card` and `mini_fruit` calls.
4. Restore the empty-state text block.
5. Restore the green box drawing:
   - fill rectangle
   - 1px border
   - 3px green left accent
6. Restore green title, muted age, and pale green body text.
7. Restore the old `y = 267 + index * 82` spacing and `460 × 75` card size.
8. Leave `src/stickers.rs`, sticker draw order, and all sticker kinds untouched.
9. Build, restart the renderer, send a notification, and inspect an encoded frame.
10. Commit after visual verification.

## What stays the same

- `POST /api/notifications`, `GET /api/notifications`, and
  `DELETE /api/notifications` behavior.
- 32 retained notifications, newest first.
- Up to three notifications drawn at once.
- Title limit 48, message limit 320, newlines only in the message.
- Age format: `now`, `Nm`, `Nh`, `Nd`.
- Header latest-title line, clock, status metrics, and sticker draw order.
- All sticker rendering, including the fruit timer, countdown, todo, and memo.

## Acceptance criteria

- Notification cards are dark green boxes with a muted green border and green
  left accent.
- No pastel fill, white sticker border, shadow, or fruit icon appears on a
  notification card.
- Titles are green, bodies are pale green, and ages are muted green.
- The empty state matches the green-box dashboard identity.
- Notifications remain ephemeral; the notification API is unchanged.
- Fruit stickers are unchanged and still visually distinct.
- Rust tests pass.
- A captured HLS frame shows the restored notification boxes and unchanged fruit
  stickers.

## Risks and rollback

This is a low-risk, single-file visual change. If the green boxes do not fit the
current board, adjust only notification colors/geometry, not sticker visuals.
Rollback is a revert of `src/notifications.rs`.

## Decisions

- Restore the old empty-state copy exactly for visual consistency.
- Keep the age label at the old fixed `x = 878` position unless it clips.
- Leave the current header text and fallback alone; this plan changes only the
  notification cards.
