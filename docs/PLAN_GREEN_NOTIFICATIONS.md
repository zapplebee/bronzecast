# Plan: Restore green-box notifications

Status: Implemented and verified with empty and populated encoded frames
Scope: notification rendering and fixed dashboard text layout
Related: [Unify stickers](PLAN_UNIFY_STICKERS.md), [README](../README.md)

## Problem

Notifications currently use the green-box identity, but the dashboard still
spends much of the board on fixed labels, status tiles, and duplicated text. The
clock/date also sit in the right panel, competing with agent-controlled stickers
and notification cards.

Keep only the clock and date as fixed dashboard text, moving them above Mog.
Agent-created notifications remain visible as compact green-box cards on the
right. Fruit stickers retain their own visual language.

## Goals

- Restore the green-box notification cards.
- Move the clock/date above Mog and remove the fixed character header, greeting,
  timezone/progress text, status tiles, notification heading, footer, and empty
  notification copy.
- Place notification cards at the top-right; leave that region blank when there
  are no notifications.
- Remove fruit icons, pastel fills, white sticker borders, and sticker shadows
  from notification cards.
- Keep notifications ephemeral and in-memory.
- Keep the notification API, limits, ordering, age labels, and count unchanged.
- Keep the clock/date content, character rendering, and sticker layer unchanged;
  move only the clock/date and remove fixed dashboard chrome.
- Keep the fruit sticker layer visually distinct from notifications.

## Non-goals

- Do not make notifications persistent.
- Do not change notification validation or MCP tools.
- Do not change the notification API or validation.
- Do not change sticker visuals or behavior.
- Do not add browser overlays.

## Target visual specification

Notification cards return to the pre-sticker design:

| Property | Value |
| --- | --- |
| Card position | `x = 470`, `y = 24 + index * 82` |
| Card size | `460 × 75` |
| Fill | `rgba(4, 12, 7, 245)` (near-black green ink) |
| Border | 1px `rgba(81, 113, 66, 255)` (muted green) |
| Left accent | 3px wide `rgba(157, 208, 104, 255)` (green) |
| Title | 17px green, `x = 486`, baseline `y + 22`, wrap width 340, max 1 line |
| Age | 13px muted green, right side, baseline `y + 22` |
| Body | 16px `rgba(183, 198, 165, 255)` (pale green), `x = 486`, baseline `y + 43 + line * 18`, wrap width 428, max 2 lines |
| Empty state | No text or card; leave the notification area empty |

Render the 12-hour clock with seconds/meridiem centered in the 440px left region
above Mog: main clock baseline y=68 at 52px, seconds at y=48 at 17px, meridiem at
y=67 at 13px. Center the host-local date at baseline y=101 at 18px. Keep only
those clock/date strings as fixed dashboard text. Remove the `NOTIFICATIONS / NEWEST
FIRST` label and all fixed status/header/footer text. Cards retain their title,
age, and message, which are agent-provided content or derived card metadata.

The current word-aware wrapping in `src/notifications.rs` is better than the old
character-by-character wrapper. Keep the word-aware wrapper, but use the old card
geometry and widths.

## Implementation steps

1. Update `src/notifications.rs` for the compact composition; remove the now-unused
   notification-only `mini_fruit` helper from `src/sticker_art.rs` without changing
   sticker drawing.
2. Delete the `cards` peach/lemon/mint palette.
3. Delete the empty-state `sticker_card` and `mini_fruit` calls.
4. Leave the empty notification region blank.
5. Restore the green box drawing:
   - fill rectangle
   - 1px border
   - 3px green left accent
6. Restore green title, muted age, and pale green body text.
7. Use `y = 24 + index * 82` spacing and `460 × 75` card size.
8. Move the clock/date above Mog; remove all other fixed dashboard copy and status
   tiles. Leave sticker data, visuals, and draw order untouched.
9. Build, restart the renderer, send a notification, and inspect an encoded frame.
10. Commit after visual verification.

## What stays the same

- `POST /api/notifications`, `GET /api/notifications`, and
  `DELETE /api/notifications` behavior.
- 32 retained notifications, newest first.
- Up to three notifications drawn at once.
- Title limit 48, message limit 320, newlines only in the message.
- Age format: `now`, `Nm`, `Nh`, `Nd`.
- Clock/date content, moved above Mog, and sticker draw order.
- Agent-created notification text remains visible in the cards.
- All sticker rendering, including the fruit timer, countdown, todo, and memo.

## Acceptance criteria

- Notification cards are dark green boxes with a muted green border and green
  left accent.
- No pastel fill, white sticker border, shadow, or fruit icon appears on a
  notification card.
- Titles are green, bodies are pale green, and ages are muted green.
- No notification card or empty-state copy appears when the notification queue is
  empty; the sticker layer remains empty until agents create stickers.
- Clock/date appear above Mog; no greeting, status tiles, heading, or footer remains.
- Notifications remain ephemeral; the notification API is unchanged.
- Fruit stickers are unchanged and still visually distinct.
- Rust tests pass.
- A captured HLS frame shows the restored notification boxes and unchanged fruit
  stickers.

## Risks and rollback

This is a low-risk renderer composition change. If the green boxes do not fit the
current board, adjust only notification colors/geometry/position, not sticker
visuals. Rollback is a revert of the notification-layout commit.

## Decisions

- Leave the notification area empty when there are no notifications; draw no
  fixed empty-state copy.
- Keep the age label at the old fixed `x = 878` position unless it clips.
- Keep only clock/date as fixed UI text and place both above Mog.
- Leave the playout fallback card alone.
