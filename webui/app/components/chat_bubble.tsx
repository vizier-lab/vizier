import type { Chat, Choice } from '~/interfaces/chat'
import ReactMarkdown from 'react-markdown'
import DotLoader from './dot_loader'
import { motion, type Transition } from 'motion/react'

import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import Avatar from './avatar'
import { FiCopy, FiCheck } from 'react-icons/fi'
import { useToastStore } from '~/hooks/toastStore'

interface ChatBubbleProps {
  chat: Chat,
}

const ChatBubble = (props: ChatBubbleProps) => {
  const isAgent = props.chat.user_type === 'agent'
  const { addToast } = useToastStore()
  const copyContent = props.chat.content === 'thinking' ? '' : props.chat.content

  const username = (
    <div className="font-bold">
      {props.chat.username}
    </div>
  )

  const Profile = () => {
    return (
      <div className="flex items-center">
        <div className="rounded-full w-10 h-10 shadow-md border-solid">
          <Avatar name={props.chat.user_id || ''} rounded={false} />
        </div>
      </div>
    )
  }

  const popUpAnimation = {
    initial: { scale: 0, opacity: 0 },
    animate: { scale: 1, opacity: 1 },
    // 'originX' and 'originY' can be used directly in the 'style' or as props
    style: { transformOrigin: !isAgent ? 'top right' : 'top left' },
    transition: { type: 'spring', stiffness: 260, damping: 20 },
  }


  return (
    <div
      className={`flex ${isAgent ? 'justify-start' : 'justify-end'} items-start w-full`}
    >
      {
        props.chat.choice
          ? <div className='p-5 prose mb-5 mt-5 min-w-full border-l-2 border-dashed bg-gray-300 shadow-md relative'>
            <button
              onClick={() => {
                navigator.clipboard.writeText(formatChoice(props.chat.choice!))
                addToast('success', 'Copied!', 'Message copied to clipboard')
              }}
              className="absolute top-2 right-2 p-2 hover:bg-black/10 rounded cursor-pointer"
              title="Copy to clipboard"
            >
              <FiCopy size={14} />
            </button>
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {`**${props.chat.username}** ${formatChoice(props.chat.choice)}`}
            </ReactMarkdown>
          </div>
          : <motion.div
            initial={popUpAnimation.initial}
            animate={popUpAnimation.animate}
            style={popUpAnimation.style}
            transition={popUpAnimation.transition as Transition<any>}
            className={`m-5 mt-0 mb-10 ${isAgent ? 'bg-gray-300 justify-start' : 'bg-white justify-end'} rounded-4xl p-5 flex items-start shadow-md w-fit overflow-hidden wrap-break-word`}
          >
            {isAgent && <Profile />}
            <div
              className={`m-5 mt-0 mb-0 flex flex-col ${!isAgent ? 'items-end' : 'items-start'}`}
            >
              {username}
              <div className="prose">
                {props.chat.content === 'thinking' ? (
                  <div className="flex items-center">
                    <div className="mr-4 font-bold">thinking</div>
                    <DotLoader />
                  </div>
                ) : (
                  <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                    {props.chat.content}
                  </ReactMarkdown>
                )}
              </div>
              {props.chat.content !== 'thinking' && isAgent && (
                <button
                  onClick={() => {
                    navigator.clipboard.writeText(props.chat.content)
                    addToast('success', 'Copied!', 'Message copied to clipboard')
                  }}
                  className="mt-2 p-1 hover:bg-black/10 rounded cursor-pointer flex items-center gap-1 text-xs text-black/50"
                  title="Copy to clipboard"
                >
                  <FiCopy size={12} />
                </button>
              )}
              {props.chat.timestamp && (
                <div className="text-black opacity-50 text-xs mt-5">
                  {props.chat.timestamp}
                </div>
              )}
            </div>
            {!isAgent && <Profile />}
          </motion.div>
      }
    </div>
  )
}

const formatChoice = (choice: Choice) => {
  console.log({ choice })
  let titleMap: any = {
    think: "is **thinking**: "
  }

  let title = titleMap[choice.name] ?? `use **${choice.name}**`
  let content = `\`\`\`js\n${JSON.stringify(choice.args, null, 2)}\n\`\`\``

  if (choice.name == "think")
    content = choice.args.thought

  if (choice.name == "memory_write")
    content = choice.args.content

  if (choice.name == "python_interpreter")
    content = `\`\`\`py\n${choice.args.script}\n\`\`\``

  return `${title} \n ${content}`
}

export default ChatBubble
