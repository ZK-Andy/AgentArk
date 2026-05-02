// useChatSplitter - pointer-driven horizontal splitter for the chat pane.
//
// Extracted from ArkOrbitPage so the page itself stays focused on
// orchestration (orbits + canvas + chat). Width clamps live with the hook
// so UX rules don't drift if the splitter is reused elsewhere.

import { useCallback, useRef, useState } from "react";

const MIN_CHAT_WIDTH = 280;
const MAX_CHAT_RATIO = 0.6;
const DEFAULT_CHAT_WIDTH = 380;

type DragState = { pointerId: number; startX: number; startWidth: number };

export function useChatSplitter() {
  const [chatWidth, setChatWidth] = useState<number>(DEFAULT_CHAT_WIDTH);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<DragState | null>(null);

  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      dragRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startWidth: chatWidth,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [chatWidth],
  );

  const onPointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      const containerWidth =
        containerRef.current?.getBoundingClientRect().width ?? 1200;
      const next = drag.startWidth + (event.clientX - drag.startX);
      const max = Math.max(MIN_CHAT_WIDTH, containerWidth * MAX_CHAT_RATIO);
      setChatWidth(Math.min(max, Math.max(MIN_CHAT_WIDTH, next)));
    },
    [],
  );

  const onPointerUp = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      dragRef.current = null;
      try {
        event.currentTarget.releasePointerCapture(event.pointerId);
      } catch {
        // already released
      }
    },
    [],
  );

  return {
    chatWidth,
    containerRef,
    handlers: { onPointerDown, onPointerMove, onPointerUp },
  };
}
