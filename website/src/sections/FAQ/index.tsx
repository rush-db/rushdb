import { AnimatePresence, motion } from 'framer-motion'
import { Section } from '~/components/Section'

import { useState } from 'react'
import cx from 'classnames'
import { ChevronDown, ChevronUp } from 'lucide-react'
import Link from 'next/link'
import { Button } from '~/components/Button'

type TFaqBlock = {
  question: string
  answer: string
  link?: {
    url: string
    text: string
  }
}
const faqData: Array<TFaqBlock> = [
  {
    question: 'Is it like MongoDB?',
    answer:
      'RushDB can be likened to MongoDB for its capacity to store nested data (resembling MongoDB\'s "documents") and shares certain API features. \n\n However, RushDB features goes far beyond and stands apart by eliminating the need for configurations, data modeling and deployments, as it effortlessly utilizes any (even complex and nested) data automatically and suggests its types.\n\n What\'s more, you can dive into working with RushDB without the necessity of an SDK or specialized driver.',
    link: {
      url: 'https://docs.rushdb.com/mongodb-comparison',
      text: 'Learn more at documentation'
    }
  },
  {
    question: 'How does RushDB differ from Firebase or Supabase?',
    answer:
      'RushDB aspires to deliver an exceptional developer experience, offering a seamless end-to-end backend solution with zero configuration required. Think of it as "Data to API in a snap". \n\n In contrast to Firebase and Supabase, which often demand hours of reading and setup, RushDB simplifies the process, enabling you to get started with just a few clicks. Its intuitive API architecture maintains a consistent structure for all operations. \n\n Furthermore, RushDB offers built-in features like the Dynamic API for pinpoint search capabilities and automated data normalization, complete with type suggestions. This eliminates the need for any manual data modeling, saving you valuable time and effort.'
  },
  {
    question: 'How secure is RushDB API?',
    answer:
      'RushDB utilizes a multi-tenant architecture, ensuring that each customer has a dedicated database for data isolation. This means that your data is kept separate from other users, providing enhanced privacy and security. In addition, RushDB employs industry-standard encryption techniques to safeguard your data both during transit and when it is at rest.'
  },
  {
    question: 'Which platforms does RushDB support?',
    answer:
      "RushDB is designed to be platform-agnostic, allowing it to be utilized on various platforms and with any programming language that supports network requests and the JSON format. This flexibility enables developers to integrate RushDB seamlessly into their existing systems and leverage its capabilities regardless of the technology stack they are using. \n\n Whether you're working with web applications, mobile apps, or even IoT devices, RushDB can be seamlessly incorporated into your project, making it highly adaptable and compatible with different development environments."
  }
  // {
  //   question: 'Can RushDB be hosted on-premises?',
  //   answer: 'Yes! Schedule a call with us to explore your requirements.',
  //   link: {
  //     url: 'https://calendly.com/collect-so/collect-intro',
  //     text: 'Schedule a call'
  //   }
  // }
]

const Accordion = ({ question, answer, link }: TFaqBlock) => {
  const [isOpen, setIsOpen] = useState(false)

  return (
    <>
      <motion.div
        initial={false}
        key="question"
        className="related font-open relative z-20 cursor-pointer rounded-br-md rounded-tr-md border-l-2 px-1 py-2 shadow-sm"
        onClick={() => setIsOpen(!isOpen)}
      >
        <motion.div className="typography-xl flex items-center justify-between">
          {question}
          {isOpen ?
            <ChevronUp />
          : <ChevronDown />}
        </motion.div>
      </motion.div>
      <AnimatePresence initial={false}>
        {isOpen && (
          <motion.div
            key="content"
            initial="collapsed"
            animate="open"
            exit="collapsed"
            className={cx('typography-base w-full')}
            variants={{
              open: { height: 'auto', opacity: 1 },
              collapsed: { height: 0, opacity: 0 }
            }}
            transition={{ duration: 0.3, damping: 100 }}
          >
            <div style={{ whiteSpace: 'pre-line' }}>{answer}</div>
            {link ?
              <Link href={link.url} target="_blank">
                <Button className="mt-4" variant="outline">
                  {link.text}
                </Button>
              </Link>
            : null}
          </motion.div>
        )}
      </AnimatePresence>
    </>
  )
}

export function FAQSection() {
  return (
    <Section className="" data-theme="dark">
      <div className="container mb-16 pb-32 text-center">
        <h2 className="typography-3xl mb-16">Frequently Asked Questions</h2>
        <div className="m-auto grid max-w-4xl grid-cols-1 gap-5 px-5 text-left">
          {faqData.map((data) => (
            <Accordion {...data} key={data.question} />
          ))}
        </div>
      </div>
    </Section>
  )
}
