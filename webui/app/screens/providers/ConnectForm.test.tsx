import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { ProviderSummary } from '../../api/endpoints/catalogue'
import { ConnectForm } from './ConnectForm'

type Provider = Parameters<typeof ConnectForm>[0]['provider']

/** The exact policy the server's guide carries (contract `provider-credentials-api.md` §1). */
const POLICY = `{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["bedrock:InvokeModel", "bedrock:ListFoundationModels"],
    "Resource": "*"
  }]
}`

const GUIDE: NonNullable<ProviderSummary['guide']> = {
    intro: 'Use an IAM access key pair with permission to invoke Bedrock models.',
    steps: [
        { title: 'Create an IAM access key', body: 'IAM → Users → Security credentials → Create access key.' },
        { title: 'Attach the minimal policy', body: 'Allow the actions below.' },
        { title: 'Enable model access', body: 'Bedrock → Model access.' },
        { title: 'Choose a region', body: 'Use a Bedrock region where the model is offered.' },
        { title: 'Paste the values', body: 'Enter the access key ID, secret access key and region.' },
    ],
    policy: { language: 'json', code: POLICY },
    links: [
        { label: 'AWS IAM users', url: 'https://console.aws.amazon.com/iam/home#/users' },
        { label: 'Bedrock model access', url: 'https://console.aws.amazon.com/bedrock/home#/modelaccess' },
        { label: 'AWS supported models', url: 'https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html' },
        { label: 'Vizier provider docs', url: 'https://www.vizier.rs/docs/configuration/providers#amazon-bedrock' },
    ],
}

function bedrock(guide: ProviderSummary['guide']): Provider {
    return {
        id: 'amazon-bedrock',
        name: 'Amazon Bedrock',
        fields: ['access_key_id', 'secret_access_key', 'session_token', 'region'],
        endpoint: null,
        doc: null,
        guide,
    }
}

function draw(editing: boolean, guide: ProviderSummary['guide'] = GUIDE) {
    return renderToString(<ConnectForm provider={bedrock(guide)} editing={editing} onDone={() => {}} onCancel={() => {}} />)
}

const escaped = (text: string) => text.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')

describe('the provider setup guide', () => {
    it('shows the five steps, the exact policy and the four links for a new connection', () => {
        const html = draw(false)
        expect(html).toContain('How to connect')
        for (const step of GUIDE.steps) expect(html).toContain(step.title)
        expect(html).toContain(escaped(POLICY))
        expect(html).toContain('bedrock:InvokeModel')
        expect(html).toContain('bedrock:ListFoundationModels')
        for (const link of GUIDE.links) {
            expect(html).toContain(link.label)
            expect(html).toContain(link.url)
        }
        expect(html).toContain('Access key ID')
        expect(html).toContain('Secret access key')
        expect(html).toContain('Session token (optional)')
        expect(html).toContain('Region')
    })

    it('stays reachable while editing, collapsed rather than removed', () => {
        const html = draw(true)
        expect(html).toContain('How to connect')
        expect(html).toContain('Show')
        // Collapsed: the policy is not drawn until the person expands the guide. The section is
        // outside the save/result notices, so a failed save or test never hides it.
        expect(html).not.toContain(escaped(POLICY))
        expect(html).not.toContain('Saved, but the connection check failed')
    })

    it('renders no guide for a provider without one', () => {
        const html = draw(false, null)
        expect(html).not.toContain('How to connect')
        expect(html).toContain('Region')
    })
})
