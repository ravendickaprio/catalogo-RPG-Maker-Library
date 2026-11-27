import { useEffect, useState, type ReactNode } from "react";
import { coverUrl, useCoverTag } from "../lib/cover";

interface Props {
  id: number;
  hasCover: boolean;
  alt: string;
  /** Rendered when there is no cover or it failed to load. */
  placeholder: (failed: boolean) => ReactNode;
  className?: string;
}

/** Cover served by the `cover` protocol: native lazy loading, falls back to the placeholder on error. */
export default function CoverImage({ id, hasCover, alt, placeholder, className }: Props) {
  const tag = useCoverTag(id);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [id, tag, hasCover]);
  if (!hasCover || failed) return <>{placeholder(failed)}</>;
  return <img className={className} src={coverUrl(id, tag)} alt={alt} loading="lazy" decoding="async" draggable={false} onError={() => setFailed(true)} />;
}
