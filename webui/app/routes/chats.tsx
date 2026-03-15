import { useEffect, useRef, useState } from 'react'
import { useLocation, useParams } from 'react-router'
import ChatBubble from '~/components/chat_bubble'
import Editor from '~/components/editor'
import type { AgentDetail, Chat, WSChatResponse } from '~/interfaces/chat'
import { base_url, getAgentDetail } from '~/services/vizier'
import useWebSocket from 'react-use-websocket'
import { FaChevronDown } from 'react-icons/fa'
import { useSessionStore } from '~/hooks/sessionStore'

const Chat = () => {
  const [chats, setChats] = useState<Chat[]>([])
  const [agentDetail, setAgentDetail] = useState<AgentDetail | null>(null)
  const [isThinking, setIsThinking] = useState(false)
  const { agentId } = useParams()
  const username = useSessionStore((state: any) => state.username)

  const sessionUrl = `ws://${base_url}/api/v1/agents/${agentId}/session/${agentId}/chat`
  const { sendJsonMessage, lastJsonMessage, getWebSocket } = useWebSocket(sessionUrl)

  useEffect(() => {
    if (!agentId) return

    setChats([])
    setIsThinking(false)

    getAgentDetail(agentId).then((res: any) => setAgentDetail(res.data.data))
  }, [agentId])

  let bottomRef: any = useRef(null)
  const toBottom = () => {
    bottomRef.current?.scrollIntoView({ behaviour: 'smooth' })
  }

  useEffect(() => {
    if (!lastJsonMessage) return


    let res: WSChatResponse = lastJsonMessage as WSChatResponse

    setIsThinking(res.thinking)

    if (res.thinking !== isThinking) {
      toBottom()
    }
    if (res.thinking) return

    let newChat: Chat = {
      user_id: agentId,
      username: agentDetail?.name,
      user_type: 'agent',
      content: res.content,
      timestamp: new Date().toISOString(),
    }

    setChats([...chats, newChat])
  }, [lastJsonMessage])

  const location = useLocation()

  const send = (content: string) => {
    let newChat: Chat = {
      user_id: username,
      username: username,
      user_type: 'user',
      content: content,
      timestamp: new Date().toISOString(),
    }

    sendJsonMessage({
      user: username,
      content,
    })
    setChats([...chats, newChat])
    toBottom()
  }

  useEffect(() => {
    const initialPrompt = location.state?.prompt
    if (initialPrompt) send(initialPrompt)
  }, [])

  return (
    <div
      id="chatroom"
      className="h-full w-full flex flex-col justify-between relative overflow-hidden"
    >
      <div className="w-full overflow-y-scroll no-scrollbar p-5">
        {chats.map((chat, i) => (
          <ChatBubble key={i} chat={chat} />
        ))}
        {isThinking && (
          <ChatBubble chat={{ user_id: agentDetail?.agent_id, username: agentDetail?.name, user_type: 'agent', content: 'thinking' }} />
        )}
        <div id="end" className="h-[25vh]" ref={bottomRef} />
      </div>
      <div className="absolute w-full flex flex-col justify-between bottom-0 p-5">
        <div></div>
        <div className="flex items-center h-full">
          <Editor onSubmit={send} />
          <div
            className="w-15 h-15 bg-white flex justify-center items-center ml-5 shadow-md hover:shadow-xl rounded-full"
            onClick={() => toBottom()}
          >
            <FaChevronDown />
          </div>
        </div>
      </div>
    </div>
  )
}

export default Chat
