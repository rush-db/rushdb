export const generateJsonLd = (type: string, data: any) => {
  switch (type) {
    case 'homepage': {
      return {
        '@context': 'https://schema.org',
        '@type': 'WebSite',
        name: 'RushDB',
        url: 'https://rushdb.com'
      }
    }

    case 'blogRoot': {
      return {
        '@context': 'https://schema.org',
        '@type': 'Blog',
        name: 'RushDB Blog',
        url: 'https://rushdb.com/blog',
        description: 'Insights, guides, and technical deep dives on graph databases, SaaS, and AI.',
        publisher: {
          '@type': 'Organization',
          name: 'RushDB',
          logo: {
            '@type': 'ImageObject',
            url: 'https://rushdb.com/logo.png'
          }
        }
      }
    }

    case 'blog': {
      return {
        '@context': 'https://schema.org',
        '@type': 'BlogPosting',
        headline: data.title,
        description: data.description,
        datePublished: data.datePublished,
        dateModified: data.dateModified || data.datePublished,
        author: {
          '@type': 'Person',
          name: 'Artemiy Vereshchinskiy'
        },
        publisher: {
          '@type': 'Organization',
          name: 'RushDB',
          logo: {
            '@type': 'ImageObject',
            url: 'https://rushdb.com/logo.png'
          }
        },
        mainEntityOfPage: {
          '@type': 'WebPage',
          '@id': data.url
        }
      }
    }

    case 'pricing': {
      return {
        '@context': 'https://schema.org',
        '@type': 'WebPage',
        name: 'Pricing - RushDB',
        description: 'Compare RushDB plans and find the best solution for your needs.',
        url: 'https://rushdb.com/pricing',
        mainEntityOfPage: {
          '@type': 'WebPage',
          '@id': 'https://rushdb.com/pricing'
        }
      }
    }

    case 'legal': {
      return {
        '@context': 'https://schema.org',
        '@type': 'WebPage',
        name: data.name,
        url: data.url,
        description: `${data.name} page of RushDB. Read our policies here.`,
        mainEntityOfPage: {
          '@type': 'WebPage',
          '@id': data.url
        }
      }
    }

    case 'faq': {
      return {
        '@context': 'https://schema.org',
        '@type': 'FAQPage',
        mainEntity: data.questions.map((q: any) => ({
          '@type': 'Question',
          name: q.question,
          acceptedAnswer: {
            '@type': 'Answer',
            text: q.answer
          }
        }))
      }
    }

    case 'breadcrumb': {
      return {
        '@context': 'https://schema.org',
        '@type': 'BreadcrumbList',
        itemListElement: data.items.map((item: any, index: number) => ({
          '@type': 'ListItem',
          position: index + 1,
          name: item.name,
          item: item.url
        }))
      }
    }

    default: {
      return null
    }
  }
}
