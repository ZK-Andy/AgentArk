import { createTheme } from "@mui/material";

export const appTheme = createTheme({
  palette: {
    mode: "dark",
    primary: {
      main: "#2fd4ff"
    },
    secondary: {
      main: "#14f195"
    },
    background: {
      default: "#030711",
      paper: "#091527"
    },
    text: {
      primary: "#ecf5ff",
      secondary: "#9bb4d6"
    }
  },
  shape: {
    borderRadius: 14
  },
  typography: {
    fontFamily: "'Space Grotesk', 'IBM Plex Sans', 'Segoe UI', sans-serif",
    h3: {
      fontWeight: 700
    },
    h5: {
      fontWeight: 600
    }
  },
  components: {
    MuiCard: {
      styleOverrides: {
        root: {
          border: "1px solid rgba(106, 150, 198, 0.22)",
          background: "linear-gradient(140deg, rgba(9,21,39,0.92), rgba(9,21,39,0.72))",
          backdropFilter: "blur(6px)"
        }
      }
    },
    MuiButton: {
      styleOverrides: {
        root: {
          textTransform: "none",
          fontWeight: 600
        }
      }
    }
  }
});
