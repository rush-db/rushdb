import as from '../../images/team/as.png'
import av from '../../images/team/av.png'
import Image from 'next/image'

export const Mission = () => {
  return (
    <section className="container mx-auto grid max-w-3xl content-center items-center gap-4 py-20">
      <div className="flex items-center justify-start gap-4 md:justify-between">
        <h3 className="typography-2xl md:typography-xl font-bold">Team & Mission</h3>
        <div className="flex">
          <Image src={av.src} alt="Artemiy" width={60} height={60} className="rounded-full" loading="lazy" />
          <Image
            src={as.src}
            alt="Andrey"
            width={60}
            height={60}
            className="ml-[-10px] rounded-full"
            loading="lazy"
          />
        </div>
      </div>
      <p className="text-content3 mb2 text-md md:text-base">
        We're two engineers with 15+ years of combined experience at Yandex, 3Commas, and Sumsub. Throughout
        our careers, we've seen how data management consistently holds teams back - from complex feature
        additions to database operations and search functionality. Every project required meticulous data
        planning, normalization, and custom retrieval solutions, creating unnecessary friction and delays.
      </p>
      <p className="text-content3 text-md md:text-base">
        RushDB exists to eliminate these barriers. Our mission is to make developers unstoppable by radically
        simplifying data operations. We're building the foundation that lets engineering teams move at maximum
        velocity - no more wrestling with databases, search implementations, or data restructuring. Just pure,
        frictionless development focused on shipping features and scaling ideas.
      </p>
      <p className="text-content3 text-md md:text-base">
        Developer experience is our north star. We believe great software shouldn't be bottlenecked by data
        infrastructure. By handling the complex data layer, we free developers to focus on what truly matters:
        creating exceptional products that deliver real value.
      </p>
    </section>
  )
}
