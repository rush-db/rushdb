import { Section, SectionHeader, SectionSubtitle, SectionTitle } from '~/components/Section'

export function BenchmarksSection() {
  return (
    <Section className="container">
      <SectionHeader>
        <SectionTitle>Performance at a Glance</SectionTitle>
        <SectionSubtitle>
          From query speeds to data processing times, see how RushDB stands up to the demands of modern
          development environments. Whether it's handling vast datasets or ensuring swift data retrieval, our
          performance insights demonstrate our commitment to speed, stability, and scalability.
        </SectionSubtitle>
      </SectionHeader>
      TODO
    </Section>
  )
}
