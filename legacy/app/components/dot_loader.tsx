'use client'

import { motion, type Variants } from 'motion/react'

function DotLoader() {
  const dotVariants: Variants = {
    pulse: {
      y: [-5, 5, -5],
      transition: {
        duration: 1.2,
        repeat: Infinity,
        ease: 'easeInOut',
      },
    },
  }

  const words = 'thinking...'

  return (
    <motion.div
      animate="pulse"
      transition={{ staggerChildren: 0.2 }}
      className="container w-fit"
    >
      <motion.div
        className="dot bg-black rounded-full"
        variants={dotVariants}
      />
      <motion.div
        className="dot bg-black rounded-full"
        variants={dotVariants}
      />
      <motion.div
        className="dot bg-black rounded-full"
        variants={dotVariants}
      />

      <StyleSheet />
    </motion.div>
  )
}

function StyleSheet() {
  return (
    <style>
      {`
            .container {
                display: flex;
                justify-content: center;
                align-items: center;
                gap: 0.5em;
            }

            .dot {
                width: 0.20em;
                height: 0.20em;
                will-change: transform;
            }
            `}
    </style>
  )
}

export default DotLoader
