// Calcolo di alba/tramonto per coordinate/data, interamente locale (nessuna
// chiamata a servizi esterni: la posizione dell'utente non lascia mai il
// browser). Formula astronomica standard (base NOAA/SunCalc, di dominio
// pubblico) — verificata numericamente contro orari reali di Roma.

const RAD = Math.PI / 180;
const DAY_MS = 1000 * 60 * 60 * 24;
const J1970 = 2440588;
const J2000 = 2451545;
const OBLIQUITY = RAD * 23.4397;

const toJulian = (date: Date) => date.valueOf() / DAY_MS - 0.5 + J1970;
const fromJulian = (j: number) => new Date((j + 0.5 - J1970) * DAY_MS);
const toDays = (date: Date) => toJulian(date) - J2000;

const declination = (l: number, b: number) =>
  Math.asin(Math.sin(b) * Math.cos(OBLIQUITY) + Math.cos(b) * Math.sin(OBLIQUITY) * Math.sin(l));
const solarMeanAnomaly = (d: number) => RAD * (357.5291 + 0.98560028 * d);
const eclipticLongitude = (m: number) => {
  const c = RAD * (1.9148 * Math.sin(m) + 0.02 * Math.sin(2 * m) + 0.0003 * Math.sin(3 * m));
  const p = RAD * 102.9372;
  return m + c + p + Math.PI;
};
const julianCycle = (d: number, lw: number) => Math.round(d - 0.0009 - lw / (2 * Math.PI));
const approxTransit = (ht: number, lw: number, n: number) => 0.0009 + (ht + lw) / (2 * Math.PI) + n;
const solarTransitJ = (ds: number, m: number, l: number) =>
  J2000 + ds + 0.0053 * Math.sin(m) - 0.0069 * Math.sin(2 * l);
const hourAngle = (h: number, phi: number, d: number) =>
  Math.acos((Math.sin(h) - Math.sin(phi) * Math.sin(d)) / (Math.cos(phi) * Math.cos(d)));

const SUNRISE_SUNSET_ANGLE = RAD * -0.833;

export interface SunTimes {
  sunrise: Date;
  sunset: Date;
}

export function getSunTimes(date: Date, latitude: number, longitude: number): SunTimes {
  const lw = RAD * -longitude;
  const phi = RAD * latitude;
  const d = toDays(date);
  const n = julianCycle(d, lw);
  const ds = approxTransit(0, lw, n);
  const m = solarMeanAnomaly(ds);
  const l = eclipticLongitude(m);
  const dec = declination(l, 0);
  const jNoon = solarTransitJ(ds, m, l);

  const w = hourAngle(SUNRISE_SUNSET_ANGLE, phi, dec);
  const a = approxTransit(w, lw, n);
  const jSet = solarTransitJ(a, m, l);
  const jRise = jNoon - (jSet - jNoon);

  return { sunrise: fromJulian(jRise), sunset: fromJulian(jSet) };
}

/** true se, alle coordinate date, in questo momento è giorno (tra alba e tramonto). */
export function isDaytime(now: Date, latitude: number, longitude: number): boolean {
  const { sunrise, sunset } = getSunTimes(now, latitude, longitude);
  return now >= sunrise && now < sunset;
}
