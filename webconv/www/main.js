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

    /** @type {{[x: string]: StateConfig}} */
    states;

    /** @type {string | null} */
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

class Side {

    /** @type {string} */
    name;

    /** @type {HTMLElement} */
    pane;

    /** @type {HTMLTextAreaElement} */
    contents;

    /** @type {HTMLElement} */
    error;

    /**
     * @param {string} name
     */
    constructor(name) {
        let pane = document.getElementById(name + "--pane");
        if (pane === null) {
            throw new Error("unreachable");
        }
        let contents = pane.querySelector(".contents");
        if (contents === null || !(contents instanceof HTMLTextAreaElement)) {
            throw new Error("unreachable");
        }
        let error = pane.querySelector('.error');
        if (error == null || !(error instanceof HTMLElement)) {
            throw new Error("unreachable");
        }
        this.name = name;
        this.pane = pane;
        this.contents = contents;
        this.error = error;
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
 * @returns {"struct" | "map_tree"}
 */
function getInterRepr() {
    let value = getInputValue("inter_repr");
    if (!(value == "struct" || value == "map_tree")) {
        throw new Error("unreachable");
    }
    return value;
}

async function main() {
    await desynced_lib.default();
    let encoded_state = new Side("encoded");
    let decoded_state = new Side("decoded");
    /**
     * @param {Side} side
     * @returns {StateConfig}
     */
    function setupState(side) {
        let {name, contents, error} = side;
        return {
            enter({value}) {
                error.innerHTML = "";
                if (value !== undefined) {
                    contents.value = value;
                }
                document
                    .querySelectorAll("[data-show_state=" + name + "]")
                    .forEach((el) => el.removeAttribute('hidden'));
                contents.setSelectionRange(0, 0);
                contents.focus();
            },
            exit() {
                document
                    .querySelectorAll("[data-show_state=" + name + "]")
                    .forEach((el) => el.setAttribute('hidden', ""));
            },
        }
    }
    let state = new StateMaintainer({
        "encoded": setupState(encoded_state),
        "decoded": setupState(decoded_state),
    }, "encoded");
    state.states.decoded.exit();
    state.states.encoded.enter({value: ""});
    document.querySelectorAll('.stage__loading')
        .forEach((el) => el.setAttribute('hidden', ''));
    document.querySelectorAll('.stage__main')
        .forEach((el) => el.removeAttribute('hidden'));
    function decode() {
        let {contents, error: errors} = encoded_state;
        state.switchState(() => {
            try {
                return {value: desynced_lib.decode(
                    contents.value.trim(),
                    {
                        decodeFormat: getDecodeFormat(),
                        decodeStyle: getDecodeStyle(),
                        interRepr: getInterRepr(),
                    },
                )};
            } catch (error) {
                errors.innerHTML = error;
                throw error;
            }
        }, "encoded", "decoded")
    }
    function encode() {
        let {contents, error: errors} = decoded_state;
        state.switchState(() => {
            try {
                return {value: desynced_lib.encode(
                    contents.value.trim(),
                    {
                        decodeFormat: getDecodeFormat(),
                        interRepr: getInterRepr(),
                    },
                )};
            } catch (error) {
                errors.innerHTML = error;
                throw error;
            }
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
        (button) => button.addEventListener('click', () => decode())
    );
    encoded_state.contents.addEventListener('keydown', (event) => {
        if (event.key == "Enter" && event.ctrlKey) {
            decode();
        }
    });
    document.querySelectorAll("button[data-action=cv_to_encoded]").forEach(
        (button) => button.addEventListener('click', () => encode())
    );
    decoded_state.contents.addEventListener('keydown', (event) => {
        if (event.key == "Enter" && event.ctrlKey) {
            encode();
        }
    });
}

main();

