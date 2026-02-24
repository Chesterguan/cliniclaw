import { redirect } from "next/navigation";

/**
 * /chart/[patientId] — redirect to the Notes tab.
 *
 * The chart root has no content of its own. Per clinical workflow convention
 * (Epic Hyperspace opens on the Storyboard → Notes is the first clinical tab),
 * we redirect to /notes while preserving the encounter query parameter.
 *
 * Next.js 15: params and searchParams are Promises in server components.
 */
export default async function ChartRootPage({
  params,
  searchParams,
}: {
  params: Promise<{ patientId: string }>;
  searchParams: Promise<{ encounter?: string }>;
}) {
  const { patientId } = await params;
  const { encounter } = await searchParams;
  const encSuffix = encounter ? `?encounter=${encounter}` : "";
  redirect(`/chart/${patientId}/notes${encSuffix}`);
}
