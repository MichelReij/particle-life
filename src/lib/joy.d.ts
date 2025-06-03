// Type definitions for JoyStick library
// Based on: https://github.com/bobboteck/JoyStick

interface JoyStickData {
    xPosition: number;
    yPosition: number;
    cardinalDirection: string;
    x: number;
    y: number;
}

interface JoyStickOptions {
    title?: string;
    width?: number;
    height?: number;
    internalFillColor?: string;
    internalLineWidth?: number;
    internalStrokeColor?: string;
    externalLineWidth?: number;
    externalStrokeColor?: string;
    autoReturnToCenter?: boolean;
}

type JoyStickCallback = (data: JoyStickData) => void;

declare class JoyStick {
    constructor(
        container: string,
        options?: JoyStickOptions,
        callback?: JoyStickCallback
    );
    GetPosX(): number;
    GetPosY(): number;
    GetDir(): string;
    GetX(): number;
    GetY(): number;
}

declare global {
    interface Window {
        JoyStick: typeof JoyStick;
    }
}

export = JoyStick;
export as namespace JoyStick;
