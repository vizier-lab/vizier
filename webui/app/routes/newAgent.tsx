import { useEffect, useState } from "react"
import { agentSchema } from "~/services/vizier";

export default function NewAgent() {
  let [schema, setSchema] = useState<any>({});
  let [step, setStep] = useState(1);
  let [form, setForm] = useState({});

  useEffect(() => {
    agentSchema().then((res: any) => {
      setSchema(res.data)
    })
  }, [])



  let to_title = (key: any) => {
    return key.split("_").map((word: string) => word.charAt(0).toUpperCase() + word.slice(1)).join(' ')
  }

  let render_input = (key: any, prop: any) => {
    let input = < ></>

    if (key.endsWith("timeout") || key.endsWith("interval")) {
      input = <>
        <input type="number" /> &nbsp; secs
      </>;
      return input;
    }

    if (prop.type) {

      if (prop.type == 'string' || prop.type?.[0] == 'string') {
        input = <input />
      }

      if (prop.type == 'number' || prop.type?.[0] == 'string') {
        input = <input type="number" />
      }

      if (prop.type == 'integer' || prop.type?.[0] == 'integer') {
        input = <input type="number" />
      }

      if (prop.type == 'boolean' || prop.type?.[0] == 'boolean') {
        input = <input type="checkbox" />
      }

      if (prop.enum) {
        input = <select>
          {prop.enum.map((item: any) => <option key={item} className="bg-black">{item}</option>)}
        </select>
      }

      if (prop.type == 'object' || prop.type?.[0] == 'object') {
        input = <div className="grid grid-cols-2">
          {Object.keys(prop.properties).filter(key => prop.properties[key].type !== 'null').map(key => {
            return <div className="mb-4!">
              <div className="mb-2! text-l font-bold">{to_title(key)}</div>
              <div>
                {render_input(key, prop.properties[key])}
              </div>
            </div>
          })}
        </div>
      }
    } else {

      let ref = prop["$ref"].replace("#/$defs/", "");
      let def = schema["$defs"][ref]
      input = render_input(key, def)
    }

    return input
  }


  let render_step_1 = () => {
    if (!schema?.properties) {
      return <></>
    }

    console.log('>>', schema)

    let agent_information =
      <div className="p-5! px-7! mx-0! m-5! bg-surface rounded-2xl">
        <h1 className="mb-2!">Agent Information</h1>
        <div className="grid grid-cols-2">
          {
            Object.keys(schema.properties)
              .filter((key: any) => schema.properties[key].type)
              .map(key => {
                let prop = schema.properties[key];

                let input = render_input(key, prop)

                return <div className="mb-4!">
                  <div className="mb-2! font-bold">{to_title((key))}</div>

                  <div>{input}</div>
                </div>
              })
          }
        </div>

      </div >



    return <div>{agent_information} {
      Object.keys(schema.properties)
        .filter((key: any) => key !== "tools" && !schema.properties[key].type)
        .map(key => {
          let prop = schema.properties[key];

          let input = render_input(key, prop)

          return <div className="p-5! px-7! mx-0! m-5! bg-surface rounded-2xl">
            <div className="text-xl font-bold mb-2!">{to_title((key))}</div>

            <div>{input}</div>
          </div>
        })

    }</div>
  }

  let render_step_2 = () => {
    if (!schema?.properties) {
      return <></>
    }

    let ref = schema.properties['tools']["$ref"].replace("#/$defs/", "");
    let def = schema["$defs"][ref]
    let input = render_input('tools', def)


    return <div>
      {Object.keys(def.properties).map(key => {
        let prop = def.properties[key]
        let input = render_input(key, prop)
        return <div className="bg-surface p-5! px-7! mx-0! m-4! rounded-2xl">
          <div className="text-xl font-bold mb-2!">{to_title((key))}</div>
          <div>{input}</div>
        </div>
      })}
    </div>
  }


  let stepForm = [
    render_step_1,
    render_step_2,
  ];

  return <>
    <div className="main-header">
      <div>
        <h3 style={{ margin: 0 }}>New Agents</h3>
      </div>
    </div>
    <div className="w-full h-full overflow-scroll! p-5! pt-0!">
      <div>
        {stepForm[step - 1]()}
      </div>
      <div className="flex justify-between">
        {
          step > 1 ?
            <button className="btn btn-secondary" onClick={() => setStep(step - 1)} >
              Prev
            </button> : <div></div>
        }

        {step < stepForm.length ?
          <button className="btn btn-secondary" onClick={() => setStep(step + 1)} >
            Next
          </button> :
          <button className="btn btn-primary" onClick={() => setStep(step + 1)} >
            Submit
          </button>

        }
      </div>

    </div >

  </>
}
