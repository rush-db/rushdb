import { VideoBlock } from "~/sections/Dashboard/VideoBlock"
import { Section, SectionHeader, SectionSubtitle } from "~/components/Section"
import cx from "classnames"

export const Dashboard = () => {
  return (
    <Section className="container max-w-6xl">
      <SectionHeader className="text-center">
        <h3 className={cx("typography-3xl mb-0 md:text-2xl")}>
          Fully-Featured{" "}
          <i>
            <span className="font-special text-[56px] md:text-[48px]">
              Dashboard
            </span>
          </i>
        </h3>
        <SectionSubtitle className="m-auto max-w-4xl">
          Advanced dashboard makes managing your data easy and efficient. With
          powerful search tools and intuitive controls, you can quickly find
          what you need and optimize your workflow, all in one place.
        </SectionSubtitle>
      </SectionHeader>

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-2 md:gap-6 xl:gap-8">
        <VideoBlock src="/videos/setup.mp4" />
        <VideoBlock src="/videos/create.mp4" />
        <VideoBlock src="/videos/search.mp4" />
        <VideoBlock src="/videos/delete.mp4" />
      </div>
    </Section>
  )
}
