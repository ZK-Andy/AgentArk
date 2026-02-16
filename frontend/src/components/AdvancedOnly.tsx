import type { ReactNode } from "react";

type Props = {
  showAdvanced: boolean;
  children: ReactNode;
  fallback?: ReactNode;
};

export function AdvancedOnly({ showAdvanced, children, fallback = null }: Props) {
  if (!showAdvanced) {
    return fallback ? <>{fallback}</> : null;
  }
  return <>{children}</>;
}
