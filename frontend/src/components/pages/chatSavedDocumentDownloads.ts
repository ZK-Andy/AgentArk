export type SavedDocumentDownloadTarget = {
  id: string;
  filename: string;
  downloadUrl: string;
};

type DocumentRow = Record<string, unknown>;

const SAVED_DOCUMENT_FILENAME_PATTERN =
  /[A-Za-z0-9][A-Za-z0-9 _.[\]()@+-]{0,180}\.[A-Za-z0-9]{1,12}/;

function valueString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function cleanSavedDocumentFilename(value: string): string {
  return value
    .trim()
    .replace(/^["'`]+|["'`]+$/g, "")
    .replace(/[),.;:!?]+$/g, "")
    .trim();
}

function addUniqueFilename(out: string[], value: string): void {
  const filename = cleanSavedDocumentFilename(value);
  if (!filename || !SAVED_DOCUMENT_FILENAME_PATTERN.test(filename)) return;
  if (!out.some((existing) => existing.toLowerCase() === filename.toLowerCase())) {
    out.push(filename);
  }
}

export function savedDocumentFilenamesFromAssistantText(text: string): string[] {
  const source = (text || "").trim();
  if (!source || !/\bDocuments\b/i.test(source) || !/\bsaved\b/i.test(source)) {
    return [];
  }

  const filenames: string[] = [];
  const inlineCodePattern =
    /\bsaved\s+(?:as|to)\s+`([^`\r\n]+)`\s+(?:in|to)\s+(?:your\s+)?Documents\b/gi;
  let match: RegExpExecArray | null = null;
  while ((match = inlineCodePattern.exec(source)) !== null) {
    addUniqueFilename(filenames, match[1] || "");
  }

  const plainPattern =
    /\bsaved\s+(?:as|to)\s+([^`\r\n]+?)\s+(?:in|to)\s+(?:your\s+)?Documents\b/gi;
  while ((match = plainPattern.exec(source)) !== null) {
    const raw = match[1] || "";
    const filenameMatch = raw.match(SAVED_DOCUMENT_FILENAME_PATTERN);
    if (filenameMatch?.[0]) addUniqueFilename(filenames, filenameMatch[0]);
  }

  return filenames;
}

export function resolveSavedDocumentDownload(
  candidateFilenames: string[],
  documents: DocumentRow[],
): SavedDocumentDownloadTarget | null {
  const normalizedCandidates = candidateFilenames
    .map((filename) => cleanSavedDocumentFilename(filename).toLowerCase())
    .filter(Boolean);
  if (normalizedCandidates.length === 0) return null;

  for (const candidate of normalizedCandidates) {
    for (const document of documents) {
      const filename = valueString(document.filename);
      const downloadUrl = valueString(document.download_url);
      if (!filename || !downloadUrl) continue;
      if (filename.toLowerCase() !== candidate) continue;
      return {
        id: valueString(document.id),
        filename,
        downloadUrl,
      };
    }
  }

  return null;
}
