import type { Recipe } from './index'

export const form: Recipe = {
    intent: 'A short form: inputs with rules, a check step, and a button that sends the values to you as the person\'s message.',
    notes: [
        'Bind each input with $bindState; the values live in state under /form.',
        'checks show under a field after the person leaves it; validateForm checks every field and writes {valid, errors} to /formValidation.',
        'Send only appears once the form is valid. sendMessage works only from the person\'s own click, at most once a second.',
        'data carries the values to you; they reach you as the content of the person\'s message, so say in text what you want from it.',
    ],
    spec: {
        root: 'page',
        state: { form: { name: '', email: '', plan: 'Team', notify: true } },
        elements: {
            page: { type: 'Card', props: { title: 'Request access', subtitle: 'We reply within a day' }, children: ['fields', 'actions'] },
            fields: { type: 'Stack', props: { gap: 'md' }, children: ['name', 'email', 'plan', 'notify'] },
            name: {
                type: 'TextInput',
                props: { label: 'Name', value: { $bindState: '/form/name' }, checks: [{ type: 'required', message: 'Enter your name' }] },
            },
            email: {
                type: 'TextInput',
                props: {
                    label: 'Email',
                    value: { $bindState: '/form/email' },
                    checks: [{ type: 'required', message: 'Enter your email' }, { type: 'email', message: 'That is not an email address' }],
                },
            },
            plan: { type: 'Select', props: { label: 'Plan', value: { $bindState: '/form/plan' }, options: ['Free', 'Team', 'Business'] } },
            notify: { type: 'Switch', props: { label: 'Email me updates', value: { $bindState: '/form/notify' } } },
            actions: { type: 'Row', props: { gap: 'sm' }, children: ['check', 'send'] },
            check: { type: 'Button', props: { label: 'Check', variant: 'secondary' }, on: { press: { action: 'validateForm', params: {} } } },
            send: {
                type: 'Button',
                props: { label: 'Send', variant: 'primary' },
                visible: { $state: '/formValidation/valid' },
                on: { press: { action: 'sendMessage', params: { text: 'Access request submitted', data: { $state: '/form' } } } },
            },
        },
    },
}
