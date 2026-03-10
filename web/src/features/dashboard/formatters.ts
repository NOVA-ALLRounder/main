type DashboardDateValue = Date | number | string | null | undefined;

const shortTimeFormatter = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
});

const longTimeFormatter = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
});

const monthDayTimeFormatter = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
});

const numericMonthDayTimeFormatter = new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
});

function toDate(value: DashboardDateValue) {
    if (value == null) return null;
    return value instanceof Date ? value : new Date(value);
}

function formatDashboardDate(
    formatter: Intl.DateTimeFormat,
    value: DashboardDateValue
): string {
    const date = toDate(value);
    return date ? formatter.format(date) : "—";
}

export function formatDashboardShortTime(value: DashboardDateValue) {
    return formatDashboardDate(shortTimeFormatter, value);
}

export function formatDashboardLongTime(value: DashboardDateValue) {
    return formatDashboardDate(longTimeFormatter, value);
}

export function formatDashboardMonthDayTime(value: DashboardDateValue) {
    return formatDashboardDate(monthDayTimeFormatter, value);
}

export function formatDashboardNumericMonthDayTime(value: DashboardDateValue) {
    return formatDashboardDate(numericMonthDayTimeFormatter, value);
}
