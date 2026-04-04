import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router'
import { FiSearch, FiX } from 'react-icons/fi'
import { useMemoryStore } from '../hooks/memoryStore'
import { motion, AnimatePresence } from 'motion/react'
import type { Memory } from '../interfaces/types'

export default function MemorySearch() {
  const { agentId } = useParams()
  const navigate = useNavigate()
  const { searchQuery, setSearchQuery, searchResults, isSearching, performSearch, clearSearch } = useMemoryStore()
  const [isExpanded, setIsExpanded] = useState(false)
  const [localQuery, setLocalQuery] = useState('')

  // Debounced search
  useEffect(() => {
    const timer = setTimeout(() => {
      if (localQuery.trim() && agentId) {
        performSearch(agentId, localQuery)
      } else {
        clearSearch()
      }
    }, 300)

    return () => clearTimeout(timer)
  }, [localQuery, agentId, performSearch, clearSearch])

  const handleSelectMemory = (slug: string) => {
    if (agentId) {
      navigate(`/${agentId}/memory`)
      // Could open the specific memory in edit/view mode
      clearSearch()
      setLocalQuery('')
      setIsExpanded(false)
    }
  }

  if (!agentId) return null

  return (
    <div className="relative">
      <div className={`flex items-center gap-2 transition-all duration-200 ${isExpanded ? 'w-full' : 'w-auto'}`}>
        <div className={`relative flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 ${isExpanded ? 'flex-1' : ''}`}>
          <FiSearch className="text-gray-400" size={16} />
          <input
            type="text"
            value={localQuery}
            onChange={(e) => {
              setLocalQuery(e.target.value)
              setSearchQuery(e.target.value)
              setIsExpanded(true)
            }}
            onBlur={() => {
              // Delay closing to allow clicking results
              setTimeout(() => setIsExpanded(false), 200)
            }}
            placeholder="Search memories..."
            className="flex-1 bg-transparent border-none outline-none text-sm text-gray-700 dark:text-gray-200 placeholder-gray-400 min-w-0"
          />
          {localQuery && (
            <button
              onClick={() => {
                setLocalQuery('')
                clearSearch()
              }}
              className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <FiX size={14} />
            </button>
          )}
        </div>
      </div>

      <AnimatePresence>
        {isExpanded && localQuery.trim() && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            className="absolute top-full left-0 right-0 mt-2 bg-white dark:bg-gray-900 rounded-lg shadow-xl border border-gray-200 dark:border-gray-700 overflow-hidden z-50"
          >
            {isSearching ? (
              <div className="p-4 text-center text-sm text-gray-500">Searching...</div>
            ) : searchResults.length > 0 ? (
              <div className="max-h-80 overflow-y-auto">
                {searchResults.map((memory: Memory) => (
                  <button
                    key={memory.slug}
                    onClick={() => handleSelectMemory(memory.slug)}
                    className="w-full px-4 py-3 text-left hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors border-b border-gray-100 dark:border-gray-800 last:border-none"
                  >
                    <div className="font-medium text-sm text-gray-900 dark:text-gray-100">
                      {memory.title}
                    </div>
                    <div className="text-xs text-gray-500 mt-1">
                      {memory.slug}
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <div className="p-4 text-center text-sm text-gray-500">
                No memories found
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
