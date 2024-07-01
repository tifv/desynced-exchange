import * as desynced_lib from "./lib/desynced_exchange_web.js";

/**
 * @typedef {{
 *     enter: StateEnter,
 *     exit: StateExit,
 * }} StateConfig
 */

/**
 * @typedef {{
 *    value?: string,
 * }} ActionResult
 */

/**
 * @typedef {(result: ActionResult) => undefined} StateEnter
*/

/**
 * @typedef {() => undefined} StateExit
*/

/**
 * @typedef {() => ActionResult | Promise<ActionResult>} Action
*/

class StateMaintainer {
    /**
     * @type {{[x: string]: StateConfig}}
     */
    states;

    /**
     * @type {string | null}
     */
    state;

    /**
     * @param {{[x: string]: StateConfig}} states
     * @param {string} startState
     */
    constructor(states, startState) {
        this.states = states;
        this._checkState(startState);
        this.state = startState;
    }
    /**
     * @param {string} state
     */
    _checkState(state) {
        if (state === null) {
            throw new Error("Valid state cannot be null");
        }
        if (this.states[state] === undefined) {
            throw new Error("Undefined state " + state);
        }
    }
    /**
     * @param {Action} action
     * @param {string} oldState
     * @param {string} newState
     */
    async switchState(action, oldState, newState) {
        this._checkState(oldState);
        this._checkState(newState);
        if (this.state !== oldState) {
            throw new Error("Out-of-order action");
        }
        this.state = null;
        /**
         * @type {{value?: string}}
         */
        let result;
        try {
            result = await action();
            this.states[oldState].exit.call(this);
            this.states[newState].enter.call(this, result);
            this.state = newState;
        } catch (error) {
            this.state = oldState;
            throw error;
        }
        return result;
    }
}

/**
 * @param {string} name
 * @returns {string}
 */
function getInputValue(name) {
    let inputs = Array.from(/** @type {NodeListOf<HTMLInputElement>} */(
        document
            .querySelectorAll("input[name=" + name + "]")
    )).filter((el) => el.checked);
    if (inputs.length != 1) {
        throw new Error("unreachable");
    }
    let [input] = inputs;
    return input.value;
}

/**
 * @returns {"ron" | "json"}
 */
function getDecodeFormat() {
    let value = getInputValue("decode_format");
    if (!(value == "ron" || value == "json")) {
        throw new Error("unreachable");
    }
    return value;
}

/**
 * @returns {"pretty" | "compact"}
 */
function getDecodeStyle() {
    let value = getInputValue("decode_style");
    if (!(value == "pretty" || value == "compact")) {
        throw new Error("unreachable");
    }
    return value;
}

/**
 * @returns {"struct" | "tree"}
 */
function getInterRepr() {
    let value = getInputValue("inter_repr");
    if (!(value == "struct" || value == "tree")) {
        throw new Error("unreachable");
    }
    return value;
}

async function main() {
    await desynced_lib.default();
    /**
     * @param {string} state
     * @returns {StateConfig}
     */
    function setupState(state) {
        let section = document.getElementById("pane__" + state);
        if (section === null) {
            throw new Error("unreachable");
        }
        return {
            enter({value}) {
                section.querySelectorAll('.errors')
                    .forEach((el) => el.innerHTML = "");
                if (value !== undefined) {
                    section.querySelectorAll('textarea')
                    .forEach((el) => el.value = value);
                }
                document
                    .querySelectorAll("[data-show_state=" + state + "]")
                    .forEach((el) => el.removeAttribute('hidden'));
            },
            exit() {
                document
                    .querySelectorAll("[data-show_state=" + state + "]")
                    .forEach((el) => el.setAttribute('hidden', ""));
            },
        }
    }
    let state = new StateMaintainer({
        "encoded": setupState("encoded"),
        "decoded": setupState("decoded"),
    }, "encoded");
    state.states.decoded.exit();
    state.states.encoded.enter({value: ""});
    document.querySelectorAll('.stage__loading')
        .forEach((el) => el.setAttribute('hidden', ''));
    document.querySelectorAll('.stage__main')
        .forEach((el) => el.removeAttribute('hidden'));
    function decode() {
        let input_pane = document.getElementById("pane__encoded");
        if (input_pane == null) {
            throw new Error("unreachable");
        }
        let input = input_pane.querySelector('textarea');
        if (input == null) {
            throw new Error("unreachable");
        }
        state.switchState(() => {
            return {value: desynced_lib.decode(input.value, {
                decodeFormat: getDecodeFormat(),
                decodeStyle: getDecodeStyle(),
                interRepr: getInterRepr(),
            })}
        }, "encoded", "decoded")
    }
    function encode() {
        let input_pane = document.getElementById("pane__decoded");
        if (input_pane == null) {
            throw new Error("unreachable");
        }
        let input = input_pane.querySelector('textarea');
        if (input == null) {
            throw new Error("unreachable");
        }
        state.switchState(() => {
            return {value: desynced_lib.encode(input.value, {
                decodeFormat: getDecodeFormat(),
                interRepr: getInterRepr(),
            })}
        }, "decoded", "encoded")
    }
    document.querySelectorAll("button[data-action=go_to_decoded]").forEach(
        (button) =>
        button.addEventListener('click', () => {
            state.switchState(() => ({}), "encoded", "decoded")
        })
    );
    document.querySelectorAll("button[data-action=go_to_encoded]").forEach(
        (button) =>
        button.addEventListener('click', () => {
            state.switchState(() => ({}), "decoded", "encoded")
        })
    );
    document.querySelectorAll("button[data-action=cv_to_decoded]").forEach(
        (button) => button.addEventListener('click', decode)
    );
    document.querySelectorAll("button[data-action=cv_to_encoded]").forEach(
        (button) => button.addEventListener('click', encode)
    );
}

main();

