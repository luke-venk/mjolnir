import {
  circleFieldDimsForThrowType,
  Infraction,
  ThrowEvent,
  ThrowType,
} from "./schemas";
import { v4 as uuidv4 } from "uuid";

function randomInfractions(): { type: Infraction; confidence: number }[] {
  const infractions: { type: Infraction; confidence: number }[] = [];
  if (Math.random() < 0.3) {
    infractions.push({
      type: Infraction.LEFT_SECTOR,
      confidence: Math.random(),
    });
  }
  if (Math.random() < 0.3) {
    infractions.push({
      type: Infraction.RIGHT_SECTOR,
      confidence: Math.random(),
    });
  }
  if (Math.random() < 0.3) {
    infractions.push({ type: Infraction.CIRCLE, confidence: Math.random() });
  }
  return infractions;
}

export async function getThrowEvent(throwType: ThrowType): Promise<ThrowEvent> {
  // If in fake-data mode, whether or not the simulated throw data is generated
  // by Next.js or Axum depends on if it's in frontend-only dev mode (Next.js
  // generates it) or integration/prod mode (Axum generates it).
  const useExternalApi = process.env.NEXT_PUBLIC_USE_EXTERNAL_API === "true";

  // URL varies based on whether or not the request is same origin (frontend-only
  // dev, production, etc.) or if request is across servers (integration-dev).
  // If in fake-data mode, call this HTTP route to get the simulated throw from
  // the backend.
  const urlGetAnalyzeThrow =
    process.env.NEXT_PUBLIC_API_BASE_URL + "/api/analyze-throw";

  if (useExternalApi) {
    const res = await fetch(urlGetAnalyzeThrow);
    if (!res.ok) {
      console.error("Failed to fetch simluated throw from backend.");
    } else {
      return await res.json();
    }
  }

  await new Promise((res) => setTimeout(res, 250)); // simulate network delay

  const { circleDiameter, fieldLength } =
    circleFieldDimsForThrowType(throwType);
  const randDistanceBase = Math.random() * fieldLength;
  const randDistance = circleDiameter / 2.0 + randDistanceBase;
  const maxTheta = throwType === ThrowType.JAVELIN ? 28.96 : 34.92;
  const normTheta = maxTheta / 2.0;
  const randPosTheta = Math.random() * normTheta;
  const randomX = randDistance * Math.cos((randPosTheta * Math.PI) / 180);
  const randomY =
    randDistance *
    Math.sin((randPosTheta * Math.PI) / 180) *
    (Math.random() < 0.5 ? -1 : 1);
  const infractionsChance = Math.random();
  // cumulative probability of 26.5% chance of infraction
  const infractions = infractionsChance < 0.4 ? randomInfractions() : [];

  return {
    throwId: uuidv4(),
    timestampFinalFrameNs: new Date().toISOString(),
    throwType,
    distanceM: randDistance,
    infractions,
    images: [
      "https://placeholdpicsum.dev/photo/800/450",
      "https://placeholdpicsum.dev/photo/1600/900",
      "https://placeholdpicsum.dev/photo/1200/675",
    ].sort(() => Math.random() - 0.5),
    landingPointXY: infractions.length ? undefined : [randomX, randomY],
  };
}
