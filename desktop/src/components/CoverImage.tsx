import { useEffect, useState } from "react";
import { coverUrl, useCoverTag } from "../lib/cover";
import defaultCover from "../assets/default-cover.webp";

interface Props {
  id: number;
  hasCover: boolean;
  alt: string;
  className?: string;
}

/**
 * Cover served by the `cover` protocol: native lazy loading. Without a cover (or if it fails to
 * load) shows the bundled default image; nothing is written to the database.
 */
export default function CoverImage({ id, hasCover, alt, className }: Props) {
  const tag = useCoverTag(id);
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [id, tag, hasCover]);
  if (!hasCover || failed) {
    return <img className={(className ? className + " " : "") + "cover-default"} src={defaultCover} alt={alt ? "Sin portada" : ""} title="Sin portada" draggable={false} />;
  }
  return <img className={className} src={coverUrl(id, tag)} alt={alt} loading="lazy" decoding="async" draggable={false} onError={() => setFailed(true)} />;
}
