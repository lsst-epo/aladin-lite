// Copyright 2015 - UDS/CNRS
// The Aladin Lite program is distributed under the terms
// of the GNU General Public License version 3.
//
// This file is part of Aladin Lite.
//
//    Aladin Lite is free software: you can redistribute it and/or modify
//    it under the terms of the GNU General Public License as published by
//    the Free Software Foundation, version 3 of the License.
//
//    Aladin Lite is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU General Public License for more details.
//
//    The GNU General Public License is available in COPYING file
//    along with Aladin Lite.
//

import { FSM } from "../FiniteStateMachine";
import { View } from "../View";
/******************************************************************************
 * Aladin Lite project
 * 
 * Class Selector
 * 
 * The circle selection finite state machine
 * 
 * Author: Matthieu Baumann[CDS]
 * 
 *****************************************************************************/

export class CircleSelect extends FSM {
    // constructor
    constructor(options, view) {
        let start = (params) => {
            const {callback} = params;
            view.setCursor('crosshair');
            view.aladin.showReticle(false)

            this.callback = callback;
            view.setMode(View.SELECT)
        }

        let mousedown = (params) => {
            const {coo} = params;
            // start a new selection
            this.startCoo = coo;
        }

        let mousemove = (params) => {
            const {coo} = params;
            this.coo = coo;

            view.requestRedraw();
        };

        let draw = () => {
            let ctx = view.catalogCtx;

            if (!view.catalogCanvasCleared) {
                ctx.clearRect(0, 0, view.width, view.height);
                view.catalogCanvasCleared = true;
            }
            // draw the selection
            ctx.fillStyle = options.color + '7f';
            ctx.strokeStyle = options.color;
            ctx.lineWidth = options.lineWidth;

            var r2 = (this.coo.x - this.startCoo.x) * (this.coo.x - this.startCoo.x) + (this.coo.y - this.startCoo.y) * (this.coo.y - this.startCoo.y);
            var r = Math.sqrt(r2);

            ctx.beginPath();
            ctx.arc(this.startCoo.x, this.startCoo.y, r, 0, 2 * Math.PI);
            ctx.fill();
            ctx.stroke();
        }

        let mouseup = (params) => {
            var x, y;
            const {coo} = params;
            this.coo = coo;
            // finish the selection
            var r2 = (this.coo.x - this.startCoo.x) * (this.coo.x - this.startCoo.x) + (this.coo.y - this.startCoo.y) * (this.coo.y - this.startCoo.y);
            var r = Math.sqrt(r2);

            x = this.startCoo.x;
            y = this.startCoo.y;
            (typeof this.callback === 'function') && this.callback({
                x, y, r,
                label: 'circle',
                contains(s) {
                    let dx = (s.x - x)
                    let dy = (s.y - y);

                    return dx*dx + dy*dy <= r2;
                },
                bbox() {
                    return {
                        x: x - r,
                        y: y - r,
                        w: 2*r,
                        h: 2*r
                    }
                }
            });

            // TODO: remove these modes in the future
            view.aladin.showReticle(true)
            view.setCursor('default');

            // execute general callback
            if (view.callbacksByEventName) {
                var callback = view.callbacksByEventName['select'];
                if (typeof callback === "function") {
                    // !todo
                    let selectedObjects = view.selectObjects(this);
                    callback(selectedObjects);
                }
            }
            view.setMode(View.PAN)
            view.requestRedraw();
        };

        let mouseout = mouseup;

        super({
            state: 'mouseup',
            transitions: {
                start: {
                    mousedown
                },
                mousedown: {
                    mousemove
                },
                mousemove: {
                    draw,
                    mouseup,
                    mouseout
                },
                draw: {
                    mousemove,
                    mouseup,
                    mouseout
                },
                mouseout: {
                    start
                },
                mouseup: {
                    start,
                }
            }
        })
    };
}