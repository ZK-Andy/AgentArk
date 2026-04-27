import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

function normalizeModuleId(id: string): string {
  return id.replace(/\\/g, "/");
}

function isNodeModule(id: string): boolean {
  return normalizeModuleId(id).includes("/node_modules/");
}

function isReactVendor(id: string): boolean {
  const normalized = normalizeModuleId(id);
  return (
    normalized.includes("/react/") ||
    normalized.includes("/react-dom/") ||
    normalized.includes("/scheduler/")
  );
}

function isMuiVendor(id: string): boolean {
  const normalized = normalizeModuleId(id);
  return normalized.includes("/@emotion/") || normalized.includes("/@mui/");
}

function isEchartsVendor(id: string): boolean {
  const normalized = normalizeModuleId(id);
  return (
    normalized.includes("/echarts-for-react/") ||
    normalized.includes("/echarts/") ||
    normalized.includes("/zrender/")
  );
}

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    sourcemap: false,
    chunkSizeWarningLimit: 1800,
    rollupOptions: {
      output: {
        strictExecutionOrder: true,
        codeSplitting: {
          includeDependenciesRecursively: false,
          groups: [
            {
              name: "vendor-react",
              test: (id: string) => isNodeModule(id) && isReactVendor(id),
              priority: 30,
              minSize: 0,
            },
            {
              name: "vendor-mui",
              test: (id: string) => isNodeModule(id) && isMuiVendor(id),
              priority: 20,
              minSize: 0,
            },
            {
              name: "vendor-echarts",
              test: (id: string) => isNodeModule(id) && isEchartsVendor(id),
              priority: 10,
              minSize: 0,
            },
          ],
        },
      },
    },
  },
});
