//! All unit definitions using the define_units! macro.

use crate::units::macros::*;
use crate::units::unit::Unit;

// Pre-calculated constants for angle conversions
const PI: f64 = std::f64::consts::PI;
const DEG_TO_RAD: f64 = PI / 180.0;
const ARCMIN_TO_RAD: f64 = PI / 10800.0;
const ARCSEC_TO_RAD: f64 = PI / 648000.0;
const REV_TO_RAD: f64 = 2.0 * PI;
const DEG2_TO_SR: f64 = DEG_TO_RAD * DEG_TO_RAD;

define_units! {
    // =========================================================================
    // SI BASE UNITS
    // =========================================================================

    /// Meter - SI base unit of length
    METER: LENGTH => 1.0, "m", ["meter", "meters"];

    /// Kilogram - SI base unit of mass
    KILOGRAM: MASS => 1.0, "kg", ["kilogram", "kilograms"];

    /// Second - SI base unit of time
    SECOND: TIME => 1.0, "s", ["second", "sec", "seconds"];

    /// Ampere - SI base unit of electric current
    AMPERE: CURRENT => 1.0, "A", ["ampere", "amp", "amperes"];

    /// Kelvin - SI base unit of temperature
    KELVIN: TEMPERATURE => 1.0, "K", ["kelvin"];

    /// Mole - SI base unit of amount of substance
    MOLE: AMOUNT => 1.0, "mol", ["mole", "moles"];

    /// Candela - SI base unit of luminous intensity
    CANDELA: LUMINOSITY => 1.0, "cd", ["candela"];

    // =========================================================================
    // SI DERIVED UNITS
    // =========================================================================

    /// Newton - SI derived unit of force
    NEWTON: FORCE => 1.0, "N", ["newton", "newtons"];

    /// Joule - SI derived unit of energy
    JOULE: ENERGY => 1.0, "J", ["joule", "joules"];

    /// Watt - SI derived unit of power
    WATT: POWER => 1.0, "W", ["watt", "watts"];

    /// Pascal - SI derived unit of pressure
    PASCAL: PRESSURE => 1.0, "Pa", ["pascal", "pascals"];

    /// Hertz - SI derived unit of frequency
    HERTZ: FREQUENCY => 1.0, "Hz", ["hertz"];

    /// Coulomb - SI derived unit of electric charge
    COULOMB: CHARGE => 1.0, "C", ["coulomb", "coulombs"];

    /// Volt - SI derived unit of electric potential
    VOLT: VOLTAGE => 1.0, "V", ["volt", "volts"];

    /// Ohm - SI derived unit of resistance
    OHM: RESISTANCE => 1.0, "\u{03A9}", ["ohm", "ohms"];

    /// Farad - SI derived unit of capacitance
    FARAD: CAPACITANCE => 1.0, "F", ["farad", "farads"];

    /// Henry - SI derived unit of inductance
    HENRY: INDUCTANCE => 1.0, "H", ["henry", "henrys"];

    /// Tesla - SI derived unit of magnetic field
    TESLA: MAGNETIC_FIELD => 1.0, "T", ["tesla"];

    /// Weber - SI derived unit of magnetic flux
    WEBER: MAGNETIC_FLUX => 1.0, "Wb", ["weber", "webers"];

    // =========================================================================
    // LENGTH
    // =========================================================================

    /// Kilometer
    KILOMETER: LENGTH => 1000.0, "km", ["kilometer", "kilometers"];

    /// Centimeter
    CENTIMETER: LENGTH => 0.01, "cm", ["centimeter", "centimeters"];

    /// Millimeter
    MILLIMETER: LENGTH => 0.001, "mm", ["millimeter", "millimeters"];

    /// Mile
    MILE: LENGTH => 1609.344, "mi", ["mile", "miles"];

    /// Yard
    YARD: LENGTH => 0.9144, "yd", ["yard", "yards"];

    /// Foot
    FOOT: LENGTH => 0.3048, "ft", ["foot", "feet"];

    /// Inch
    INCH: LENGTH => 0.0254, "in", ["inch", "inches"];

    /// Nautical mile
    NAUTICAL_MILE: LENGTH => 1852.0, "nmi", ["nautical mile"];

    /// Astronomical unit
    AU: LENGTH => 149_597_870_700.0, "AU", ["au", "astronomical unit"];

    // =========================================================================
    // TIME
    // =========================================================================

    /// Minute
    MINUTE: TIME => 60.0, "min", ["minute", "minutes"];

    /// Hour
    HOUR: TIME => 3600.0, "hr", ["hour", "h", "hours"];

    /// Day
    DAY: TIME => 86400.0, "d", ["day", "days"];

    /// Millisecond
    MILLISECOND: TIME => 0.001, "ms", ["millisecond", "milliseconds"];

    /// Microsecond
    MICROSECOND: TIME => 0.000_001, "\u{03BC}s", ["microsecond", "us", "microseconds"];

    // =========================================================================
    // MASS
    // =========================================================================

    /// Gram
    GRAM: MASS => 0.001, "g", ["gram", "grams"];

    /// Milligram
    MILLIGRAM: MASS => 0.000_001, "mg", ["milligram", "milligrams"];

    /// Metric ton (tonne)
    TONNE: MASS => 1000.0, "t", ["tonne", "tonnes"];

    /// Pound (mass)
    POUND: MASS => 0.453_592_37, "lb", ["pound", "pounds"];

    /// Ounce
    OUNCE: MASS => 0.028_349_523_125, "oz", ["ounce", "ounces"];

    // =========================================================================
    // VELOCITY
    // =========================================================================

    /// Kilometers per hour
    KMH: VELOCITY => 1000.0 / 3600.0, "km/h", ["kilometers per hour", "kmh", "kph"];

    /// Miles per hour
    MPH: VELOCITY => 1609.344 / 3600.0, "mph", ["miles per hour", "mi/h", "mi/hr"];

    /// Knot (nautical miles per hour)
    KNOT: VELOCITY => 1852.0 / 3600.0, "kn", ["knot", "knots"];

    // =========================================================================
    // AREA
    // =========================================================================

    /// Square meter
    SQUARE_METER: AREA => 1.0, "m\u{00B2}", ["square meter", "m^2", "m2"];

    /// Hectare
    HECTARE: AREA => 10_000.0, "ha", ["hectare", "hectares"];

    /// Acre
    ACRE: AREA => 4_046.856_422_4, "ac", ["acre", "acres"];

    // =========================================================================
    // VOLUME
    // =========================================================================

    /// Cubic meter
    CUBIC_METER: VOLUME => 1.0, "m\u{00B3}", ["cubic meter", "m^3", "m3"];

    /// Liter
    LITER: VOLUME => 0.001, "L", ["liter", "l", "liters", "litre", "litres"];

    /// Milliliter
    MILLILITER: VOLUME => 0.000_001, "mL", ["milliliter", "ml", "milliliters"];

    /// US gallon
    GALLON: VOLUME => 0.003_785_411_784, "gal", ["gallon", "gallons"];

    // =========================================================================
    // FORCE
    // =========================================================================

    /// Pound-force
    POUND_FORCE: FORCE => 4.448_222, "lbf", ["pound-force"];

    /// Kilonewton
    KILONEWTON: FORCE => 1000.0, "kN", ["kilonewton", "kilonewtons"];

    // =========================================================================
    // PRESSURE
    // =========================================================================

    /// Bar
    BAR: PRESSURE => 100_000.0, "bar", ["bar"];

    /// Atmosphere
    ATMOSPHERE: PRESSURE => 101_325.0, "atm", ["atmosphere", "atmospheres"];

    /// PSI (pounds per square inch)
    PSI: PRESSURE => 6_894.757_293_168_36, "psi", ["pounds per square inch"];

    // =========================================================================
    // ENERGY
    // =========================================================================

    /// Kilowatt-hour
    KWH: ENERGY => 3_600_000.0, "kWh", ["kilowatt-hour", "kwh"];

    /// Calorie (thermochemical)
    CALORIE: ENERGY => 4.184, "cal", ["calorie", "calories"];

    /// Electronvolt
    ELECTRONVOLT: ENERGY => 1.602_176_634e-19, "eV", ["electronvolt", "ev", "electronvolts"];

    // =========================================================================
    // POWER
    // =========================================================================

    /// Kilowatt
    KILOWATT: POWER => 1000.0, "kW", ["kilowatt", "kw", "kilowatts"];

    /// Horsepower (mechanical)
    HORSEPOWER: POWER => 745.699_872, "hp", ["horsepower"];

    // =========================================================================
    // DIMENSIONLESS
    // =========================================================================

    /// Non-dimensional (pure number)
    ND: DIMENSIONLESS => 1.0, "", ["non-dimensional"];

    /// Percent
    PERCENT: DIMENSIONLESS => 0.01, "%", ["percent"];

    /// Parts per million
    PPM: DIMENSIONLESS => 0.000_001, "ppm", ["parts per million"];

    /// Parts per billion
    PPB: DIMENSIONLESS => 0.000_000_001, "ppb", ["parts per billion"];

    // =========================================================================
    // ANGLES
    // =========================================================================

    /// Radian - SI derived unit of angle
    RADIAN: ANGLE => 1.0, "rad", ["radian", "radians"];

    /// Degree
    DEGREE: ANGLE => DEG_TO_RAD, "\u{00B0}", ["degree", "deg", "degrees"];

    /// Arcminute
    ARCMINUTE: ANGLE => ARCMIN_TO_RAD, "'", ["arcminute", "arcmin", "arcminutes"];

    /// Arcsecond
    ARCSECOND: ANGLE => ARCSEC_TO_RAD, "\"", ["arcsecond", "arcsec", "arcseconds"];

    /// Revolution (full turn)
    REVOLUTION: ANGLE => REV_TO_RAD, "rev", ["revolution", "revolutions"];

    // =========================================================================
    // SOLID ANGLES
    // =========================================================================

    /// Steradian - SI derived unit of solid angle
    STERADIAN: SOLID_ANGLE => 1.0, "sr", ["steradian", "steradians"];

    /// Square degree
    SQUARE_DEGREE: SOLID_ANGLE => DEG2_TO_SR, "deg\u{00B2}", ["square degree", "deg^2"];
}
