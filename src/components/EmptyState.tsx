import { PictureIcon } from "./icons";

export default function EmptyState({ onOpen }: { onOpen: () => void }) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-3 text-neutral-600">
      <PictureIcon className="w-16 h-16 opacity-20" />
      <p className="text-sm">Open an event to begin</p>
      <button
        onClick={onOpen}
        className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded text-sm font-medium text-white transition-colors"
      >
        Open Event
      </button>
    </div>
  );
}
