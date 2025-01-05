import { ComponentPropsWithoutRef, ReactNode, useRef, useState } from 'react'
import { Section, SectionHeader, SectionSubtitle } from '~/components/Section'
import cx from 'classnames'
import { CodeBlock } from '~/components/CodeBlock'
import { Chip } from '~/components/Chip'
import { UsageScenario } from '~/sections/Workflow/UsageExample'
import { CodeWrapper } from '~/sections/Workflow/CodeWrapper'
import {
  curl_3,
  ts_3,
  curl_2,
  ts_2,
  ts_2_1,
  curl_1,
  go_1,
  java_1,
  php_1,
  python_1,
  ts_3_1,
  ruby_1,
  rust_1,
  swift_1,
  ts_1,
  ts_1_1,
  rust_2,
  ruby_2,
  python_2,
  php_2,
  swift_2,
  java_2,
  go_2,
  go_3,
  ruby_3,
  python_3,
  php_3,
  java_3,
  swift_3,
  rust_3
} from '~/sections/Workflow/codeSamples'

const examples = ['Curl', 'TS', 'Python', 'Java', 'Ruby', 'Go', 'Swift', 'Rust', 'PHP'] as const

const icons: Record<(typeof examples)[number], () => ReactNode> = {
  Curl: () => <i className="devicon-bash-plain" />,
  TS: () => <i className="devicon-typescript-plain" />,
  Python: () => <i className="devicon-python-plain" />,
  Java: () => <i className="devicon-java-plain" />,
  Ruby: () => <i className="devicon-ruby-plain" />,
  Go: () => <i className="devicon-go-original-wordmark" />,
  Swift: () => <i className="devicon-swift-plain" />,
  Rust: () => <i className="devicon-rust-original" />,
  PHP: () => <i className="devicon-php-plain" />
}

const Option = ({
  className,
  selected,
  ...props
}: ComponentPropsWithoutRef<'button'> & { selected?: boolean }) => {
  return (
    <button
      className={cx(
        'flex items-center gap-2 rounded-xl p-2 text-xl transition-colors sm:text-lg',
        selected ?
          'bg-accent text-accent-contrast hover:bg-accent-hover font-semibold'
        : 'text-content hover:bg-secondary font-medium',
        className
      )}
      {...props}
    />
  )
}

// Every app starts with Create, Read, Update, Delete (CRUD). RushDB lets you define and modify your data entries with ease. Just add your data, and start building from there.

const scenarios: {
  title: string
  description: string | ReactNode
  subtitle: string | ReactNode
  cta?: string | ReactNode
  examples: Record<(typeof examples)[number], ReactNode>
}[] = [
  {
    title: 'Simple Setup',
    description:
      "Get your API token from the dashboard, and you're all set! RushDB is designed to process data of any shape.",
    subtitle: <Chip variant="purple">Setup</Chip>,
    examples: {
      TS: (
        <CodeWrapper>
          <CodeBlock code={ts_1} />
          <CodeBlock code={ts_1_1} />
        </CodeWrapper>
      ),
      Curl: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="bash" code={curl_1} />
        </CodeWrapper>
      ),
      Go: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="go" code={go_1} />
        </CodeWrapper>
      ),
      Rust: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="rust" code={rust_1} />
        </CodeWrapper>
      ),
      Ruby: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="ruby" code={ruby_1} />
        </CodeWrapper>
      ),
      Python: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="python" code={python_1} />
        </CodeWrapper>
      ),
      PHP: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="php" code={php_1} />
        </CodeWrapper>
      ),
      Swift: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="swift" code={swift_1} />
        </CodeWrapper>
      ),
      Java: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="java" code={java_1} />
        </CodeWrapper>
      )
    }
  },
  {
    title: 'Instant Records Creation',
    description:
      "Whether you're pushing a single Record or importing thousands of them, do it in milliseconds. Your data’s shape doesn’t constrain you because RushDB adapts to it on the fly.",
    subtitle: <Chip variant="yellow">Create</Chip>,
    examples: {
      TS: (
        <CodeWrapper>
          <CodeBlock code={ts_2} />
          <CodeBlock code={ts_2_1} />
        </CodeWrapper>
      ),
      Curl: (
        <CodeWrapper>
          <CodeBlock language="bash" code={curl_2} />
        </CodeWrapper>
      ),
      Go: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="go" code={go_2} />
        </CodeWrapper>
      ),
      Rust: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="rust" code={rust_2} />
        </CodeWrapper>
      ),
      Ruby: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="ruby" code={ruby_2} />
        </CodeWrapper>
      ),
      Python: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="python" code={python_2} />
        </CodeWrapper>
      ),
      PHP: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="php" code={php_2} />
        </CodeWrapper>
      ),
      Swift: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="swift" code={swift_2} />
        </CodeWrapper>
      ),
      Java: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="java" code={java_2} />
        </CodeWrapper>
      )
    }
  },
  {
    title: 'Powerful Querying',
    description: (
      <>
        Precisely fetch any piece of data regardless of its complexity. Thanks to graph architecture and algos
        behind. Build complex queries effortlessly using Related Search capabilities,
        <code className="text-content">$and</code>, <code className="text-content">$or</code>,{' '}
        <code className="text-content">$xor</code>, <code className="text-content">$nor</code>,{' '}
        <code className="text-content">$not</code>, operators and others.
      </>
    ),
    subtitle: <Chip variant="green">Read</Chip>,
    examples: {
      TS: (
        <CodeWrapper>
          <CodeBlock code={ts_3} />
          <CodeBlock code={ts_3_1} />
        </CodeWrapper>
      ),
      Curl: (
        <CodeWrapper>
          <CodeBlock language="bash" code={curl_3} />
        </CodeWrapper>
      ),
      Go: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="go" code={go_3} />
        </CodeWrapper>
      ),
      Rust: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="rust" code={rust_3} />
        </CodeWrapper>
      ),
      Ruby: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="ruby" code={ruby_3} />
        </CodeWrapper>
      ),
      Python: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="python" code={python_3} />
        </CodeWrapper>
      ),
      PHP: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="php" code={php_3} />
        </CodeWrapper>
      ),
      Swift: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="swift" code={swift_3} />
        </CodeWrapper>
      ),
      Java: (
        <CodeWrapper className="m-auto">
          <CodeBlock language="java" code={java_3} />
        </CodeWrapper>
      )
    }
  }
]

export function WorkflowSection() {
  const wrapperRef = useRef<HTMLDivElement>(null)

  const [currentExample, setCurrentExample] = useState<(typeof examples)[number]>('TS')

  return (
    <Section className="container">
      <SectionHeader className="text-center">
        <h3 className={cx('typography-3xl mb-0 md:text-2xl')}>
          Integrates With{' '}
          <i>
            <span className="font-special text-[56px] md:text-[48px]">Anything</span>
          </i>
        </h3>
        <SectionSubtitle className="m-auto max-w-4xl">
          Whether you've just started or are already working on something big, RushDB seamlessly integrates
          into your existing development process. It adapts to your needs through the Dashboard, APIs, and
          SDKs.
        </SectionSubtitle>
      </SectionHeader>

      <div className="relative m-auto max-w-6xl" ref={wrapperRef}>
        <div className="divide-stroke-dark divide-y">
          {scenarios.map(({ title, description, subtitle, examples }) => (
            <UsageScenario
              title={title}
              description={description}
              key={title}
              example={examples[currentExample]}
              subtitle={subtitle}
            />
          ))}
        </div>
      </div>
      <div className="sticky bottom-0 m-auto grid max-w-6xl pb-6 md:w-fit md:max-w-fit">
        <div className="bg-fill/40 border-stroke-dark flex w-full rounded-2xl border p-1 shadow-2xl backdrop-blur-sm sm:px-0 md:col-start-1">
          {examples.map((example) => (
            <Option
              key={example}
              onClick={() => {
                setCurrentExample(example)
              }}
              selected={example === currentExample}
            >
              {icons[example]()}
            </Option>
          ))}
        </div>
      </div>
    </Section>
  )
}
