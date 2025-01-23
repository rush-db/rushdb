import as from '../../images/team/as.jpg'
import av from '../../images/team/av.png'
import Image from 'next/image'

export const Mission = () => {
  return (
    <section className="container mx-auto grid max-w-3xl content-center items-center gap-4 py-20">
      <div className="flex justify-start gap-4">
        <h3 className="text-3xl font-bold">Team & Mission</h3>
        <Image src={av.src} alt="Artemiy" width={80} height={80} className="rounded-full" />
        <Image src={as.src} alt="Andrey" width={80} height={80} className="ml-[-30px] rounded-full" />
      </div>
      <p className="text-content3 mb2 text-md">
        We're two engineers with 15+ years of combined experience at Yandex, 3Commas, and Sumsub. Throughout
        our careers, we've seen how data management consistently holds teams back - from complex feature
        additions to database operations and search functionality. Every project required meticulous data
        planning, normalization, and custom retrieval solutions, creating unnecessary friction and delays.
      </p>
      <p className="text-content3 text-md">
        RushDB exists to eliminate these barriers. Our mission is to make developers unstoppable by radically
        simplifying data operations. We're building the foundation that lets engineering teams move at maximum
        velocity - no more wrestling with databases, search implementations, or data restructuring. Just pure,
        frictionless development focused on shipping features and scaling ideas.
      </p>
      <p className="text-content3 text-md">
        Developer experience is our north star. We believe great software shouldn't be bottlenecked by data
        infrastructure. By handling the complex data layer, we free developers to focus on what truly matters:
        creating exceptional products that deliver real value.
      </p>
    </section>
  )
}
