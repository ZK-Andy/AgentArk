import { create } from "zustand";

const TOUR_COMPLETED_KEY = "agentark.tour.completed";

function isTourCompleted(): boolean {
  try {
    return window.localStorage.getItem(TOUR_COMPLETED_KEY) === "1";
  } catch {
    return false;
  }
}

function persistTourCompleted(done: boolean): void {
  try {
    window.localStorage.setItem(TOUR_COMPLETED_KEY, done ? "1" : "0");
  } catch {
    /* ignore storage failures */
  }
}

type UiState = {
  autoRefresh: boolean;
  showAdvancedByView: Record<string, boolean>;
  selectedNotificationId: string | null;
  tourActive: boolean;
  tourStep: number;
  tourCompleted: boolean;
  toggleAdvanced: (viewKey: string) => void;
  openNotification: (id: string) => void;
  closeNotification: () => void;
  startTour: () => void;
  nextTourStep: () => void;
  prevTourStep: () => void;
  skipTour: () => void;
  completeTour: () => void;
};

export const useUiStore = create<UiState>((set) => ({
  autoRefresh: true,
  showAdvancedByView: {},
  selectedNotificationId: null,
  tourActive: false,
  tourStep: 0,
  tourCompleted: isTourCompleted(),
  toggleAdvanced: (viewKey) =>
    set((s) => ({
      showAdvancedByView: {
        ...s.showAdvancedByView,
        [viewKey]: !(s.showAdvancedByView[viewKey] ?? false)
      }
    })),
  openNotification: (id) => set({ selectedNotificationId: id }),
  closeNotification: () => set({ selectedNotificationId: null }),
  startTour: () => set({ tourActive: true, tourStep: 0 }),
  nextTourStep: () => set((s) => ({ tourStep: s.tourStep + 1 })),
  prevTourStep: () => set((s) => ({ tourStep: Math.max(0, s.tourStep - 1) })),
  skipTour: () => {
    persistTourCompleted(true);
    set({ tourActive: false, tourStep: 0, tourCompleted: true });
  },
  completeTour: () => {
    persistTourCompleted(true);
    set({ tourActive: false, tourStep: 0, tourCompleted: true });
  },
}));
