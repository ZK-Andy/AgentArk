import Stack from "@mui/material/Stack";

import type { ChatStepCard, ComputerPaneFile } from "../types";
import { AGENTARK_RENDERERS, rendererIdForCard } from "../surface";
import { AppDeployView } from "./AppDeployView";
import { BrowseView } from "./BrowseView";
import { FileView } from "./FileView";
import { GenericSurfaceView } from "./GenericSurfaceView";
import { SearchView } from "./SearchView";
import { StatusView } from "./StatusView";
import { TerminalView } from "./TerminalView";

export interface SurfaceRendererProps {
  card: ChatStepCard;
  live?: boolean;
  snippetPath?: string;
  snippetContent?: string;
  workspaceFiles?: ComputerPaneFile[];
  deployFilePath?: string | null;
  deployFileContent?: string;
  deployFileCard?: ChatStepCard | null;
  deployFileLive?: boolean;
  onOpenDeployFile?: (path: string) => void;
}

export function SurfaceRenderer({
  card,
  live = false,
  snippetPath,
  snippetContent,
  workspaceFiles = [],
  deployFilePath = null,
  deployFileContent = "",
  deployFileCard = null,
  deployFileLive = false,
  onOpenDeployFile,
}: SurfaceRendererProps) {
  const rendererId = rendererIdForCard(card);

  if (rendererId === AGENTARK_RENDERERS.TERMINAL) {
    return <TerminalView card={card} live={live} />;
  }

  if (rendererId === AGENTARK_RENDERERS.FILE) {
    return (
      <FileView
        card={card}
        snippetPath={snippetPath}
        snippetContent={snippetContent}
        live={live}
      />
    );
  }

  if (rendererId === AGENTARK_RENDERERS.BROWSER) {
    return <BrowseView card={card} />;
  }

  if (rendererId === AGENTARK_RENDERERS.SEARCH) {
    return <SearchView card={card} />;
  }

  if (rendererId === AGENTARK_RENDERERS.DEPLOY) {
    return (
      <Stack spacing={1}>
        <AppDeployView
          card={card}
          workspaceFiles={workspaceFiles}
          onOpenFile={onOpenDeployFile}
        />
        {deployFileCard && deployFilePath ? (
          <FileView
            card={deployFileCard}
            snippetPath={deployFilePath}
            snippetContent={deployFileContent}
            live={deployFileLive}
          />
        ) : null}
      </Stack>
    );
  }

  if (rendererId === AGENTARK_RENDERERS.WORKING) {
    return <StatusView title={card.label || "Working"} detail={card.detail || card.summary} />;
  }

  return <GenericSurfaceView card={card} />;
}

export default SurfaceRenderer;
