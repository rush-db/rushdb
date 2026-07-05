import { KeyRound, Trash2 } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'

import { PageContent, PageHeader, PageTitle } from '~/elements/PageHeader'
import { WorkspacesLayout } from '~/features/workspaces/layout/WorkspacesLayout'
import { WorkspaceSettingsLayout } from '~/features/workspaces/layout/WorkspaceSettingsLayout'
import { Button } from '~/elements/Button'
import { CopyInput, TextField } from '~/elements/Input'
import { SelectField } from '~/elements/Select'
import { Switch } from '~/elements/Switch'
import { Skeleton } from '~/elements/Skeleton'
import { ConfirmDialog } from '~/elements/ConfirmDialog'

import {
  useCanUseSso,
  useDeleteSsoMutation,
  useSsoConfigsQuery,
  useUpsertSsoMutation
} from '~/features/sso/hooks'
import type { IdpType, SsoConfig, UpsertSsoConfigPayload } from '~/features/sso/types'
import { WORKSPACE_ROLES } from '~/features/workspaces/types'
import type { WorkspaceRole } from '~/features/workspaces/types'

const ROLE_OPTIONS = WORKSPACE_ROLES.filter((r) => r !== 'owner').map((role) => ({
  value: role,
  label: role.charAt(0).toUpperCase() + role.slice(1)
}))

const TYPE_OPTIONS: { value: IdpType; label: string }[] = [
  { value: 'saml', label: 'SAML 2.0' },
  { value: 'oidc', label: 'OIDC' }
]

interface FormState {
  type: IdpType
  enabled: boolean
  enforced: boolean
  domains: string
  defaultRole: WorkspaceRole
  samlEntityId: string
  samlSsoUrl: string
  samlCertificate: string
  oidcIssuer: string
  oidcClientId: string
  oidcClientSecret: string
}

const emptyForm = (type: IdpType): FormState => ({
  type,
  enabled: false,
  enforced: false,
  domains: '',
  defaultRole: 'developer',
  samlEntityId: '',
  samlSsoUrl: '',
  samlCertificate: '',
  oidcIssuer: '',
  oidcClientId: '',
  oidcClientSecret: ''
})

const fromConfig = (config: SsoConfig): FormState => ({
  type: config.type,
  enabled: config.enabled,
  enforced: config.enforced,
  domains: config.domains.join(', '),
  defaultRole: config.defaultRole,
  samlEntityId: config.samlEntityId ?? '',
  samlSsoUrl: config.samlSsoUrl ?? '',
  samlCertificate: config.samlCertificate ?? '',
  oidcIssuer: config.oidcIssuer ?? '',
  oidcClientId: config.oidcClientId ?? '',
  oidcClientSecret: ''
})

function CopyField({ label, value }: { label: string; value: string }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-content2">{label}</span>
      <CopyInput value={value} />
    </label>
  )
}

function ServiceProviderDetails({ config }: { config: SsoConfig }) {
  return (
    <div className="mt-6 rounded-lg border border-stroke p-5">
      <h3 className="text-sm font-semibold text-content">Values for your identity provider</h3>
      <p className="mt-1 text-sm leading-6 text-content2">
        Share these with your IT administrator when registering RushDB as a service provider.
      </p>
      <div className="mt-4 flex flex-col gap-4">
        {config.type === 'saml' ?
          <>
            <CopyField label="SP Entity ID / Audience" value={config.sp.entityId} />
            <CopyField label="ACS (Reply) URL" value={config.sp.acsUrl} />
            <CopyField label="SP Metadata URL" value={config.sp.metadataUrl} />
          </>
        : <CopyField label="Redirect URI (Callback)" value={config.sp.oidcRedirectUri} />}
      </div>
    </div>
  )
}

function SsoForm({ type, existing }: { type: IdpType; existing?: SsoConfig }) {
  const [form, setForm] = useState<FormState>(existing ? fromConfig(existing) : emptyForm(type))
  const { mutateAsync: upsert, isPending: saving } = useUpsertSsoMutation()
  const { mutateAsync: remove, isPending: deleting } = useDeleteSsoMutation()

  useEffect(() => {
    setForm(existing ? fromConfig(existing) : emptyForm(type))
  }, [existing, type])

  const set = <K extends keyof FormState>(key: K, value: FormState[K]) =>
    setForm((prev) => ({ ...prev, [key]: value }))

  const handleSave = async () => {
    const payload: UpsertSsoConfigPayload = {
      type,
      enabled: form.enabled,
      enforced: form.enforced,
      domains: form.domains
        .split(',')
        .map((d) => d.trim().toLowerCase())
        .filter(Boolean),
      defaultRole: form.defaultRole
    }
    if (type === 'saml') {
      payload.samlEntityId = form.samlEntityId || undefined
      payload.samlSsoUrl = form.samlSsoUrl || undefined
      payload.samlCertificate = form.samlCertificate || undefined
    } else {
      payload.oidcIssuer = form.oidcIssuer || undefined
      payload.oidcClientId = form.oidcClientId || undefined
      // Only send the secret when the admin typed a new one.
      if (form.oidcClientSecret) payload.oidcClientSecret = form.oidcClientSecret
    }
    await upsert(payload)
  }

  return (
    <div className="mt-6 flex max-w-2xl flex-col gap-5">
      <TextField
        label="Company email domains"
        caption="Comma-separated. Users with these email domains will sign in through this provider."
        placeholder="acme.com, acme.io"
        value={form.domains}
        onChange={(e) => set('domains', e.target.value)}
      />

      <SelectField
        label="Default role for new users"
        options={ROLE_OPTIONS}
        value={form.defaultRole}
        onChange={(e) => set('defaultRole', e.target.value as WorkspaceRole)}
      />

      {type === 'saml' ?
        <>
          <TextField
            label="IdP Entity ID"
            placeholder="https://idp.acme.com/saml/metadata"
            value={form.samlEntityId}
            onChange={(e) => set('samlEntityId', e.target.value)}
          />
          <TextField
            label="IdP SSO URL (Sign-on endpoint)"
            placeholder="https://idp.acme.com/saml/sso"
            value={form.samlSsoUrl}
            onChange={(e) => set('samlSsoUrl', e.target.value)}
          />
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-content">IdP X.509 Certificate</span>
            <textarea
              className="min-h-[120px] rounded-md border border-stroke bg-fill p-3 font-mono text-xs"
              placeholder="-----BEGIN CERTIFICATE-----"
              value={form.samlCertificate}
              onChange={(e) => set('samlCertificate', e.target.value)}
            />
          </label>
        </>
      : <>
          <TextField
            label="Issuer URL"
            caption="The OIDC discovery base URL (without /.well-known)."
            placeholder="https://acme.okta.com"
            value={form.oidcIssuer}
            onChange={(e) => set('oidcIssuer', e.target.value)}
          />
          <TextField
            label="Client ID"
            value={form.oidcClientId}
            onChange={(e) => set('oidcClientId', e.target.value)}
          />
          <TextField
            label="Client Secret"
            type="password"
            caption={
              existing?.hasOidcClientSecret ?
                'A secret is already stored. Leave blank to keep it.'
              : undefined
            }
            placeholder={existing?.hasOidcClientSecret ? '••••••••' : ''}
            value={form.oidcClientSecret}
            onChange={(e) => set('oidcClientSecret', e.target.value)}
          />
        </>
      }

      <div className="flex items-center justify-between">
        <div className="flex flex-col">
          <span className="text-sm font-medium text-content">Enable provider</span>
          <span className="text-sm text-content2">Allow users to sign in through this provider.</span>
        </div>
        <Switch checked={form.enabled} onCheckedChange={(v) => set('enabled', v)} />
      </div>

      <div className="flex items-center justify-between">
        <div className="flex flex-col">
          <span className="text-sm font-medium text-content">Enforce SSO</span>
          <span className="text-sm text-content2">
            Require users on these domains to sign in only through SSO.
          </span>
        </div>
        <Switch checked={form.enforced} onCheckedChange={(v) => set('enforced', v)} />
      </div>

      {existing ?
        <ServiceProviderDetails config={existing} />
      : null}

      <div className="mt-2 flex items-center gap-3">
        <Button variant="primary" loading={saving} onClick={handleSave}>
          {existing ? 'Save changes' : 'Create configuration'}
        </Button>
        {existing ?
          <ConfirmDialog
            title="Remove SSO configuration"
            description="Users from these domains will no longer be able to sign in through SSO. Continue?"
            handler={() => remove({ id: existing.id })}
            trigger={
              <Button variant="ghost" loading={deleting}>
                <Trash2 />
                Remove
              </Button>
            }
          />
        : null}
      </div>
    </div>
  )
}

export function WorkspaceSsoPage() {
  const canUseSso = useCanUseSso()
  const { data: configs, isPending } = useSsoConfigsQuery()
  const [type, setType] = useState<IdpType>('saml')

  const existing = useMemo(() => configs?.find((c) => c.type === type), [configs, type])

  return (
    <WorkspacesLayout>
      <WorkspaceSettingsLayout>
        <PageHeader className="items-start gap-5" contained>
          <div className="flex max-w-3xl flex-col gap-2">
            <PageTitle>
              <span className="inline-flex items-center gap-2">
                <KeyRound className="h-5 w-5" /> Single Sign-On
              </span>
            </PageTitle>
            <p className="text-sm leading-6 text-content2">
              Let your team sign in with your company identity provider over SAML 2.0 or OIDC. Users are
              provisioned into this workspace automatically on first login.
            </p>
          </div>
        </PageHeader>
        <PageContent contained>
          {!canUseSso ?
            <div className="max-w-2xl rounded-lg border border-stroke p-6 text-sm leading-6 text-content2">
              Single Sign-On is available on the <span className="font-medium text-content">Scale</span> plan
              and above. Upgrade your workspace to configure SAML or OIDC for your team.
            </div>
          : isPending ?
            <Skeleton className="h-80 w-full max-w-2xl rounded-md" />
          : <>
              <div className="inline-flex w-fit rounded-lg bg-fill2 p-1">
                {TYPE_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    onClick={() => setType(opt.value)}
                    className={
                      type === opt.value ?
                        'rounded-md bg-fill px-4 py-1.5 text-sm font-medium text-content shadow-sm'
                      : 'px-4 py-1.5 text-sm font-medium text-content2'
                    }
                  >
                    {opt.label}
                    {configs?.some((c) => c.type === opt.value && c.enabled) ? ' ·' : ''}
                  </button>
                ))}
              </div>

              {existing?.enforced ?
                <div className="mt-4 rounded-lg border border-warning/30 bg-warning/5 p-3 text-sm text-content2">
                  SSO is enforced for {existing.domains.join(', ')}. Password and social logins are disabled
                  for these domains.
                </div>
              : null}

              <SsoForm type={type} existing={existing} />
            </>
          }
        </PageContent>
      </WorkspaceSettingsLayout>
    </WorkspacesLayout>
  )
}
