import React from "react";
import { useLanguage } from "./i18n/LanguageProvider";

interface LoadingPageProps {
  status?: string;
}

// A figma-like 5-dot pulsing loader with brand colors
function FigmaDots() {
  return (
    <svg width="80" height="80" viewBox="0 0 80 80" role="img" aria-label="loading">
      <g>
        <circle cx="32" cy="20" r="9" fill="#F24E1E">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="1.2s" begin="0s" repeatCount="indefinite" />
        </circle>
        <circle cx="48" cy="20" r="9" fill="#FF7262">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="1.2s" begin="0.15s" repeatCount="indefinite" />
        </circle>
        <circle cx="32" cy="36" r="9" fill="#A259FF">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="1.2s" begin="0.3s" repeatCount="indefinite" />
        </circle>
        <circle cx="48" cy="36" r="9" fill="#0ACF83">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="1.2s" begin="0.45s" repeatCount="indefinite" />
        </circle>
        <circle cx="32" cy="52" r="9" fill="#1ABCFE">
          <animate attributeName="opacity" values="0.3;1;0.3" dur="1.2s" begin="0.6s" repeatCount="indefinite" />
        </circle>
      </g>
    </svg>
  );
}

export function LoadingPage({ status }: LoadingPageProps) {
  const { t } = useLanguage();
  const info = status ?? `${t.common.loading}...`;
  return (
    <div className="flex-1 min-h-full flex items-center justify-center select-none">
      <div className="flex flex-col items-center gap-4 p-6 text-center">
        <FigmaDots />
        <div className="text-xs text-muted-foreground/80">{info}</div>
      </div>
    </div>
  );
}
