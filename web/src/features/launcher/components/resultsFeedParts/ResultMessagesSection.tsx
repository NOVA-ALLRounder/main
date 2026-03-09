import { AnimatePresence, motion } from "framer-motion";
import { Pin } from "lucide-react";
import ReactMarkdown from "react-markdown";
import type { ResultMessagesSectionProps } from "@/features/launcher/components/resultsFeedParts/sectionTypes";

export function ResultMessagesSection({
  results,
  navigableItems,
  selectedIndex,
  markdownComponents,
  onPinResult,
}: ResultMessagesSectionProps) {
  return (
    <AnimatePresence>
      {results.map((res, i) => {
        const isSelected = navigableItems.findIndex((x) => x.id === `res-${i}`) === selectedIndex;
        return (
          <motion.div
            key={i}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            className={`launcher-feed-item p-3 rounded-lg text-gray-200 text-sm leading-relaxed transition-colors relative group ${
              isSelected ? "bg-white/10" : "bg-[#232323]"
            }`}
          >
            <ReactMarkdown components={markdownComponents}>{res.content}</ReactMarkdown>
            <button
              onClick={() => onPinResult(res.content)}
              className="absolute top-2 right-2 p-1.5 rounded-md text-gray-400 hover:text-white hover:bg-white/10 opacity-0 group-hover:opacity-100 transition-all"
              title="Pin to Widget"
            >
              <Pin className="w-4 h-4" />
            </button>
          </motion.div>
        );
      })}
    </AnimatePresence>
  );
}
